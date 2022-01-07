.section .iwram
    .global agb_rs__buffer_size
    .balign 4
agb_rs__buffer_size:
    .word 0

agb_arm_func agb_rs__mixer_add
    @ Arguments
    @ r0 - pointer to the data to be copied (u8 array)
    @ r1 - pointer to the sound buffer (i16 array which will alternate left and right channels, 32-bit aligned)
    @ r2 - playback speed (usize fixnum with 8 bits)
    @ r3 - amount to modify the left channel by (u16 fixnum with 4 bits)
    @ stack position 1 - amount to modify the right channel by (u16 fixnum with 4 bits)
    @
    @ The sound buffer must be SOUND_BUFFER_SIZE * 2 in size = 176 * 2
    push {r4-r8}

    ldr r7, [sp, #20]        @ load the right channel modification amount into r7

    cmp r7, r3               @ check if left and right channel need the same modifications
    beq same_modification

modifications_fallback:
    orr r7, r7, r3, lsl #16   @ r7 now is the left channel followed by the right channel modifications.

    mov r5, #0                   @ current index we're reading from
    ldr r8, agb_rs__buffer_size @ the number of steps left

1:
    add r4, r0, r5, asr #8    @ calculate the address of the next read from the sound buffer
    ldrsb r6, [r4]           @ load the current sound sample to r6
    add r5, r5, r2           @ calculate the position to read the next sample from

    ldr r4, [r1]             @ read the current value

    mla r4, r6, r7, r4       @ r4 += r6 * r7 (calculating both the left and right samples together)

    str r4, [r1], #4         @ store the new value, and increment the pointer

    subs r8, r8, #1          @ loop counter
    bne 1b                   @ jump back if we're done with the loop

    pop {r4-r8}
    bx lr

same_modification:
    @ check to see if this is a perfect power of 2
    @ r5 is a scratch register, r7 = r3 = amount to modify
    sub r5, r7, #1
    ands r5, r5, r7

    bne modifications_fallback @ not 0 means we need to do the full modification

    @ count leading zeros of r7 into r3
    mov r3, #0
1:
    add r3, r3, #1
    lsrs r7, r7, #1
    bne 1b

    mov r5, #0                   @ current index we're reading from
    ldr r8, agb_rs__buffer_size @ the number of steps left

1:
    add r4, r0, r5, asr #8    @ calculate the address of the next read from the sound buffer
    ldrsb r6, [r4]            @ load the current sound sample to r6
    add r5, r5, r2           @ calculate the position to read the next sample from

    ldr r4, [r1]             @ read the current value

    lsl r6, r6, #16
    orr r6, r6, lsr #16
    add r4, r4, r6, lsl r3   @ r4 += r6 << r3 (calculating both the left and right samples together)

    str r4, [r1], #4         @ store the new value, and increment the pointer

    subs r8, r8, #1          @ loop counter
    bne 1b                   @ jump back if we're done with the loop

    pop {r4-r8}
    bx lr

agb_arm_end agb_rs__mixer_add

agb_arm_func agb_rs__mixer_add_stereo
    @ Arguments
    @ r0 - pointer to the data to be copied (u8 array)
    @ r1 - pointer to the sound buffer (i16 array which will alternate left and right channels, 32-bit aligned)
    @
    @ The sound buffer must be SOUND_BUFFER_SIZE * 2 in size = 176 * 2
    push {r4-r8}

    ldr r5, =0x00000FFF

.macro mixer_add_loop_simple_stereo
    ldrsh r6, [r0], #2        @ load the current sound sample to r6

    ldr r4, [r1]             @ read the current value

    @ This is slightly convoluted, but is mainly done for performance reasons. It is better
    @ to hit ROM just once and then do 3 really simple instructions then do 2 ldrsbs however annoying
    @ this is. Also, since all this code is in IWRAM and we never hit ROM otherwise, all accesses
    @ are sequential and exactly the size of the bus to ROM (16 bits), so hopefully this will be super fast.
    @
    @ The next 3 instructions set up the current value in r6 to be in the expected format
    @ 1 = 2s complement marks (so if negative, these are all 1s, if positive these are 0s)
    @ L = the left sample
    @ R = the right sample
    @ 0 = all zeros
    @ Split into bytes
    @
    @ At this point
    @                        r6 = | 1 | 1 | L | R | where the upper bytes are 1s if L is negative. No care about R
                         @ asr #8 | 1 | 1 | 1 | L | drop R off the right hand side
    and r7, r5, r6, asr #8 @ r7 = | 0 | 0 | 1 | L | exactly what we want this to be. The mask puts the 1 as 00001111 ready for the shift later
    lsl r6, r6, #24        @ r6 = | R | 0 | 0 | 0 | drop everything except the right sample
    orr r6, r7, r6, asr #8 @ r6 = | 1 | R | 1 | L | now we have it perfectly set up

    add r4, r4, r6, lsl #4  @ r4 += r6 << 4 (calculating both the left and right samples together)

    str r4, [r1], #4         @ store the new value, and increment the pointer
.endm

    ldr r8, agb_rs__buffer_size
1:
    mixer_add_loop_simple_stereo
    mixer_add_loop_simple_stereo
    mixer_add_loop_simple_stereo
    mixer_add_loop_simple_stereo

    subs r8, r8, #4          @ loop counter
    bne 1b                   @ jump back if we're done with the loop

    pop {r4-r8}
    bx lr

agb_arm_end agb_rs__mixer_add_stereo

.macro clamp_s8 reg:req
    cmn \reg, #127
    mvnlt \reg, #127

    cmp \reg, #128
    movgt \reg, #128
.endm

agb_arm_func agb_rs__mixer_collapse
    @ Arguments:
    @ r0 = target buffer (i8)
    @ r1 = input buffer (i16) of fixnums with 4 bits of precision (read in sets of i16 in an i32)
    push {r4}

    ldr r2, agb_rs__buffer_size @ loop counter
    mov r4, r2

1:
    @ r12 = *r1; r1++
    ldr r12, [r1], #4

    lsl r3, r12, #16        @ r3 is going to be the right sample, push r12 left 16 bits first
    asr r3, r3, #20         @ move r3 back to being the correct value
    mov r12, r12, asr #20   @ r12 = left sample

    clamp_s8 r12            @ clamp the audio to 8 bit values
    clamp_s8 r3

    strb r3, [r0, r4] @ *(r0 + r4 = SOUND_BUFFER_SIZE) = r3
    strb r12, [r0], #1                @ *r0 = r12; r0++

    subs r2, r2, #1      @ r2 -= 1
    bne 1b               @ loop if not 0

    pop {r4}
    bx lr
agb_arm_end agb_rs__mixer_collapse
