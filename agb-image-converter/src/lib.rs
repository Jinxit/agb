use palette16::Palette16OptimisationResults;
use proc_macro::TokenStream;
use proc_macro2::Literal;
use syn::parse::Parser;
use syn::{parse_macro_input, punctuated::Punctuated, LitStr};
use syn::{Expr, ExprLit, Lit};

use std::path::PathBuf;
use std::{iter, path::Path, str};

use quote::{format_ident, quote, ToTokens};

mod aseprite;
mod colour;
mod config;
mod font_loader;
mod image_loader;
mod palette16;
mod rust_generator;

use image::GenericImageView;
use image_loader::Image;

use colour::Colour;

#[derive(Debug, Clone, Copy)]
pub(crate) enum TileSize {
    Tile8,
    Tile16,
    Tile32,
}

impl TileSize {
    fn to_size(self) -> usize {
        match self {
            TileSize::Tile8 => 8,
            TileSize::Tile16 => 16,
            TileSize::Tile32 => 32,
        }
    }
}

#[proc_macro]
pub fn include_gfx(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::LitStr);

    let filename = input.value();

    let root = std::env::var("CARGO_MANIFEST_DIR").expect("Failed to get cargo manifest dir");
    let path = Path::new(&root).join(&*filename);
    let parent = path
        .parent()
        .expect("Expected a parent directory for the path");

    let config = config::parse(&path.to_string_lossy());

    let module_name = format_ident!(
        "{}",
        path.file_stem()
            .expect("Expected a file stem")
            .to_string_lossy()
    );
    let include_path = path.to_string_lossy();

    let images = config.images();
    let image_code = images.iter().map(|(image_name, &image)| {
        convert_image(image, parent, image_name, &config.crate_prefix())
    });

    let module = quote! {
        mod #module_name {
            const _: &[u8] = include_bytes!(#include_path);

            #(#image_code)*
        }
    };

    TokenStream::from(module)
}

use quote::TokenStreamExt;
struct ByteString<'a>(&'a [u8]);
impl ToTokens for ByteString<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.append(Literal::byte_string(self.0));
    }
}

#[proc_macro]
pub fn include_aseprite_inner(input: TokenStream) -> TokenStream {
    let parser = Punctuated::<LitStr, syn::Token![,]>::parse_separated_nonempty;
    let parsed = match parser.parse(input) {
        Ok(e) => e,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut optimiser = palette16::Palette16Optimiser::new();
    let mut images = Vec::new();
    let mut tags = Vec::new();

    let root = std::env::var("CARGO_MANIFEST_DIR").expect("Failed to get cargo manifest dir");

    let filenames: Vec<PathBuf> = parsed
        .iter()
        .map(|s| s.value())
        .map(|s| Path::new(&root).join(&*s))
        .collect();

    for filename in filenames.iter() {
        let (frames, tag) = aseprite::generate_from_file(filename);

        tags.push((tag, images.len()));

        for frame in frames {
            let width = frame.width();
            assert!(width == frame.height() && width.is_power_of_two() && width <= 32);

            let image = Image::load_from_dyn_image(frame);
            add_to_optimiser(&mut optimiser, &image, width as usize);
            images.push(image);
        }
    }

    let optimised_results = optimiser.optimise_palettes(None);

    let (palette_data, tile_data, assignments) = palete_tile_data(&optimised_results, &images);

    let palette_data = palette_data.iter().map(|colours| {
        quote! {
            Palette16::new([
                #(#colours),*
            ])
        }
    });

    let mut pre = 0;
    let sprites = images
        .iter()
        .zip(assignments.iter())
        .map(|(f, assignment)| {
            let start: usize = pre;
            let end: usize = pre + (f.width / 8) * (f.height / 8) * 32;
            let data = ByteString(&tile_data[start..end]);
            pre = end;
            let width = f.width;
            let height = f.height;
            quote! {
                Sprite::new(
                    &PALETTES[#assignment],
                    #data,
                    Size::from_width_height(#width, #height)
                )
            }
        });

    let tags = tags.iter().flat_map(|(tag, num_images)| {
        tag.iter().map(move |tag| {
            let start = tag.from_frame() as usize + num_images;
            let end = tag.to_frame() as usize + num_images;
            let direction = tag.animation_direction() as usize;

            let name = tag.name();
            assert!(start <= end, "Tag {} has start > end", name);

            quote! {
                (#name, Tag::new(SPRITES, #start, #end, #direction))
            }
        })
    });

    let include_paths = filenames.iter().map(|s| {
        let s = s.as_os_str().to_string_lossy();
        quote! {
            const _: &[u8] = include_bytes!(#s);
        }
    });

    let module = quote! {
        #(#include_paths)*


        const PALETTES: &[Palette16] = &[
            #(#palette_data),*
        ];

        pub const SPRITES: &[Sprite] = &[
            #(#sprites),*
        ];

        const TAGS: &TagMap = &TagMap::new(
            &[
                #(#tags),*
            ]
        );

    };

    TokenStream::from(module)
}

fn convert_image(
    settings: &dyn config::Image,
    parent: &Path,
    variable_name: &str,
    crate_prefix: &str,
) -> proc_macro2::TokenStream {
    let image_filename = &parent.join(&settings.filename());
    let image = Image::load_from_file(image_filename);

    let tile_size = settings.tilesize().to_size();
    if image.width % tile_size != 0 || image.height % tile_size != 0 {
        panic!("Image size not a multiple of tile size");
    }

    let optimiser = optimiser_for_image(&image, tile_size);
    let optimisation_results = optimiser.optimise_palettes(settings.transparent_colour());

    rust_generator::generate_code(
        variable_name,
        &optimisation_results,
        &image,
        &image_filename.to_string_lossy(),
        settings.tilesize(),
        crate_prefix.to_owned(),
    )
}

fn optimiser_for_image(image: &Image, tile_size: usize) -> palette16::Palette16Optimiser {
    let mut palette_optimiser = palette16::Palette16Optimiser::new();
    add_to_optimiser(&mut palette_optimiser, image, tile_size);
    palette_optimiser
}

fn add_to_optimiser(
    palette_optimiser: &mut palette16::Palette16Optimiser,
    image: &Image,
    tile_size: usize,
) {
    let tiles_x = image.width / tile_size;
    let tiles_y = image.height / tile_size;

    for y in 0..tiles_y {
        for x in 0..tiles_x {
            let mut palette = palette16::Palette16::new();

            for j in 0..tile_size {
                for i in 0..tile_size {
                    let colour = image.colour(x * tile_size + i, y * tile_size + j);

                    palette.add_colour(colour);
                }
            }

            palette_optimiser.add_palette(palette);
        }
    }
}

fn palete_tile_data(
    optimiser: &Palette16OptimisationResults,
    images: &[Image],
) -> (Vec<Vec<u16>>, Vec<u8>, Vec<usize>) {
    let palette_data: Vec<Vec<u16>> = optimiser
        .optimised_palettes
        .iter()
        .map(|palette| {
            palette
                .clone()
                .into_iter()
                .map(|colour| colour.to_rgb15())
                .chain(iter::repeat(0))
                .take(16)
                .map(|colour| colour as u16)
                .collect()
        })
        .collect();

    let mut tile_data = Vec::new();

    for image in images {
        let tile_size = image.height;
        let tiles_x = image.width / tile_size;
        let tiles_y = image.height / tile_size;

        for y in 0..tiles_y {
            for x in 0..tiles_x {
                let palette_index = optimiser.assignments[y * tiles_x + x];
                let palette = &optimiser.optimised_palettes[palette_index];

                for inner_y in 0..tile_size / 8 {
                    for inner_x in 0..tile_size / 8 {
                        for j in inner_y * 8..inner_y * 8 + 8 {
                            for i in inner_x * 8..inner_x * 8 + 8 {
                                let colour = image.colour(x * tile_size + i, y * tile_size + j);
                                tile_data.push(palette.colour_index(colour));
                            }
                        }
                    }
                }
            }
        }
    }

    let tile_data = tile_data
        .chunks(2)
        .map(|chunk| chunk[0] | (chunk[1] << 4))
        .collect();

    let assignments = optimiser.assignments.clone();

    (palette_data, tile_data, assignments)
}

fn flatten_group(expr: &Expr) -> &Expr {
    match expr {
        Expr::Group(group) => &group.expr,
        _ => expr,
    }
}

#[proc_macro]
pub fn include_font(input: TokenStream) -> TokenStream {
    let parser = Punctuated::<Expr, syn::Token![,]>::parse_separated_nonempty;
    let parsed = match parser.parse(input) {
        Ok(e) => e,
        Err(e) => return e.to_compile_error().into(),
    };

    let all_args: Vec<_> = parsed.into_iter().collect();
    if all_args.len() != 2 {
        panic!("Include_font requires 2 arguments, got {}", all_args.len());
    }

    let filename = match flatten_group(&all_args[0]) {
        Expr::Lit(ExprLit {
            lit: Lit::Str(str_lit),
            ..
        }) => str_lit.value(),
        _ => panic!("Expected literal string as first argument to include_font"),
    };

    let font_size = match flatten_group(&all_args[1]) {
        Expr::Lit(ExprLit {
            lit: Lit::Float(value),
            ..
        }) => value.base10_parse::<f32>().expect("Invalid float literal"),
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => value
            .base10_parse::<i32>()
            .expect("Invalid integer literal") as f32,
        _ => panic!("Expected literal float or integer as second argument to include_font"),
    };

    let root = std::env::var("CARGO_MANIFEST_DIR").expect("Failed to get cargo manifest dir");
    let path = Path::new(&root).join(&*filename);

    let file_content = std::fs::read(&path).expect("Failed to read ttf file");

    let rendered = font_loader::load_font(&file_content, font_size);

    let include_path = path.to_string_lossy();

    quote!({
        let _ = include_bytes!(#include_path);

        #rendered
    })
    .into()
}

#[cfg(test)]
mod tests {
    use asefile::AnimationDirection;

    #[test]
    // These directions defined in agb and have these values. This is important
    // when outputting code for agb. If more animation directions are added then
    // we will have to support them there.
    fn directions_to_agb() {
        assert_eq!(AnimationDirection::Forward as usize, 0);
        assert_eq!(AnimationDirection::Reverse as usize, 1);
        assert_eq!(AnimationDirection::PingPong as usize, 2);
    }
}
