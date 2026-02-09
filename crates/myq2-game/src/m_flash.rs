// m_flash.rs — Monster muzzle flash offset table
// Converted from: myq2-original/game/m_flash.c
//
// This file defines the muzzle flash offsets for various monster weapon types.
// Used by both the game (to source shot locations) and the client (to position muzzle flashes).

#[rustfmt::skip]
pub const MONSTER_FLASH_OFFSET: &[[f32; 3]] = &[
    // flash 0 is not used
    [0.0, 0.0, 0.0],

    // MZ2_TANK_BLASTER_1                1
    [20.7, -18.5, 28.7],
    // MZ2_TANK_BLASTER_2                2
    [16.6, -21.5, 30.1],
    // MZ2_TANK_BLASTER_3                3
    [11.8, -23.9, 32.1],
    // MZ2_TANK_MACHINEGUN_1            4
    [22.9, -0.7, 25.3],
    // MZ2_TANK_MACHINEGUN_2            5
    [22.2, 6.2, 22.3],
    // MZ2_TANK_MACHINEGUN_3            6
    [19.4, 13.1, 18.6],
    // MZ2_TANK_MACHINEGUN_4            7
    [19.4, 18.8, 18.6],
    // MZ2_TANK_MACHINEGUN_5            8
    [17.9, 25.0, 18.6],
    // MZ2_TANK_MACHINEGUN_6            9
    [14.1, 30.5, 20.6],
    // MZ2_TANK_MACHINEGUN_7            10
    [9.3, 35.3, 22.1],
    // MZ2_TANK_MACHINEGUN_8            11
    [4.7, 38.4, 22.1],
    // MZ2_TANK_MACHINEGUN_9            12
    [-1.1, 40.4, 24.1],
    // MZ2_TANK_MACHINEGUN_10            13
    [-6.5, 41.2, 24.1],
    // MZ2_TANK_MACHINEGUN_11            14
    [3.2, 40.1, 24.7],
    // MZ2_TANK_MACHINEGUN_12            15
    [11.7, 36.7, 26.0],
    // MZ2_TANK_MACHINEGUN_13            16
    [18.9, 31.3, 26.0],
    // MZ2_TANK_MACHINEGUN_14            17
    [24.4, 24.4, 26.4],
    // MZ2_TANK_MACHINEGUN_15            18
    [27.1, 17.1, 27.2],
    // MZ2_TANK_MACHINEGUN_16            19
    [28.5, 9.1, 28.0],
    // MZ2_TANK_MACHINEGUN_17            20
    [27.1, 2.2, 28.0],
    // MZ2_TANK_MACHINEGUN_18            21
    [24.9, -2.8, 28.0],
    // MZ2_TANK_MACHINEGUN_19            22
    [21.6, -7.0, 26.4],
    // MZ2_TANK_ROCKET_1                23
    [6.2, 29.1, 49.1],
    // MZ2_TANK_ROCKET_2                24
    [6.9, 23.8, 49.1],
    // MZ2_TANK_ROCKET_3                25
    [8.3, 17.8, 49.5],

    // MZ2_INFANTRY_MACHINEGUN_1        26
    [26.6, 7.1, 13.1],
    // MZ2_INFANTRY_MACHINEGUN_2        27
    [18.2, 7.5, 15.4],
    // MZ2_INFANTRY_MACHINEGUN_3        28
    [17.2, 10.3, 17.9],
    // MZ2_INFANTRY_MACHINEGUN_4        29
    [17.0, 12.8, 20.1],
    // MZ2_INFANTRY_MACHINEGUN_5        30
    [15.1, 14.1, 21.8],
    // MZ2_INFANTRY_MACHINEGUN_6        31
    [11.8, 17.2, 23.1],
    // MZ2_INFANTRY_MACHINEGUN_7        32
    [11.4, 20.2, 21.0],
    // MZ2_INFANTRY_MACHINEGUN_8        33
    [9.0, 23.0, 18.9],
    // MZ2_INFANTRY_MACHINEGUN_9        34
    [13.9, 18.6, 17.7],
    // MZ2_INFANTRY_MACHINEGUN_10        35
    [15.4, 15.6, 15.8],
    // MZ2_INFANTRY_MACHINEGUN_11        36
    [10.2, 15.2, 25.1],
    // MZ2_INFANTRY_MACHINEGUN_12        37
    [-1.9, 15.1, 28.2],
    // MZ2_INFANTRY_MACHINEGUN_13        38
    [-12.4, 13.0, 20.2],

    // MZ2_SOLDIER_BLASTER_1            39
    [12.72, 9.24, 9.36],
    // MZ2_SOLDIER_BLASTER_2            40
    [25.32, 4.32, 22.8],
    // MZ2_SOLDIER_SHOTGUN_1            41
    [12.72, 9.24, 9.36],
    // MZ2_SOLDIER_SHOTGUN_2            42
    [25.32, 4.32, 22.8],
    // MZ2_SOLDIER_MACHINEGUN_1        43
    [12.72, 9.24, 9.36],
    // MZ2_SOLDIER_MACHINEGUN_2        44
    [25.32, 4.32, 22.8],

    // MZ2_GUNNER_MACHINEGUN_1        45
    [34.615, 4.485, 22.54],
    // MZ2_GUNNER_MACHINEGUN_2        46
    [33.465, 2.875, 23.805],
    // MZ2_GUNNER_MACHINEGUN_3        47
    [32.43, 2.875, 25.53],
    // MZ2_GUNNER_MACHINEGUN_4        48
    [32.43, 4.14, 25.3],
    // MZ2_GUNNER_MACHINEGUN_5        49
    [30.935, 2.3, 26.91],
    // MZ2_GUNNER_MACHINEGUN_6        50
    [30.475, 0.69, 23.92],
    // MZ2_GUNNER_MACHINEGUN_7        51
    [30.935, 0.575, 24.725],
    // MZ2_GUNNER_MACHINEGUN_8        52
    [33.35, 2.76, 22.425],
    // MZ2_GUNNER_GRENADE_1            53
    [5.29, -19.32, 8.395],
    // MZ2_GUNNER_GRENADE_2            54
    [5.29, -19.32, 8.395],
    // MZ2_GUNNER_GRENADE_3            55
    [5.29, -19.32, 8.395],
    // MZ2_GUNNER_GRENADE_4            56
    [5.29, -19.32, 8.395],

    // MZ2_CHICK_ROCKET_1                57
    // Original was -24.8 but corrected to 24.8 (PGM - this was incorrect in Q2)
    [24.8, -9.0, 39.0],

    // MZ2_FLYER_BLASTER_1            58
    [12.1, 13.4, -14.5],
    // MZ2_FLYER_BLASTER_2            59
    [12.1, -7.4, -14.5],

    // MZ2_MEDIC_BLASTER_1            60
    [12.1, 5.4, 16.5],

    // MZ2_GLADIATOR_RAILGUN_1        61
    [30.0, 18.0, 28.0],

    // MZ2_HOVER_BLASTER_1            62
    [32.5, -0.8, 10.0],

    // MZ2_ACTOR_MACHINEGUN_1        63
    [18.4, 7.4, 9.6],

    // MZ2_SUPERTANK_MACHINEGUN_1        64
    [30.0, 30.0, 88.5],
    // MZ2_SUPERTANK_MACHINEGUN_2        65
    [30.0, 30.0, 88.5],
    // MZ2_SUPERTANK_MACHINEGUN_3        66
    [30.0, 30.0, 88.5],
    // MZ2_SUPERTANK_MACHINEGUN_4        67
    [30.0, 30.0, 88.5],
    // MZ2_SUPERTANK_MACHINEGUN_5        68
    [30.0, 30.0, 88.5],
    // MZ2_SUPERTANK_MACHINEGUN_6        69
    [30.0, 30.0, 88.5],
    // MZ2_SUPERTANK_ROCKET_1            70
    [16.0, -22.5, 91.2],
    // MZ2_SUPERTANK_ROCKET_2            71
    [16.0, -33.4, 86.7],
    // MZ2_SUPERTANK_ROCKET_3            72
    [16.0, -42.8, 83.3],

    // --- Start Xian Stuff ---
    // MZ2_BOSS2_MACHINEGUN_L1            73
    [32.0, -40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_L2            74
    [32.0, -40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_L3            75
    [32.0, -40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_L4            76
    [32.0, -40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_L5            77
    [32.0, -40.0, 70.0],
    // --- End Xian Stuff

    // MZ2_BOSS2_ROCKET_1                78
    [22.0, 16.0, 10.0],
    // MZ2_BOSS2_ROCKET_2                79
    [22.0, 8.0, 10.0],
    // MZ2_BOSS2_ROCKET_3                80
    [22.0, -8.0, 10.0],
    // MZ2_BOSS2_ROCKET_4                81
    [22.0, -16.0, 10.0],

    // MZ2_FLOAT_BLASTER_1            82
    [32.5, -0.8, 10.0],

    // MZ2_SOLDIER_BLASTER_3            83
    [24.96, 12.12, -3.24],
    // MZ2_SOLDIER_SHOTGUN_3            84
    [24.96, 12.12, -3.24],
    // MZ2_SOLDIER_MACHINEGUN_3        85
    [24.96, 12.12, -3.24],
    // MZ2_SOLDIER_BLASTER_4            86
    [9.12, 11.16, 0.96],
    // MZ2_SOLDIER_SHOTGUN_4            87
    [9.12, 11.16, 0.96],
    // MZ2_SOLDIER_MACHINEGUN_4        88
    [9.12, 11.16, 0.96],
    // MZ2_SOLDIER_BLASTER_5            89
    [36.6, 11.88, -22.44],
    // MZ2_SOLDIER_SHOTGUN_5            90
    [36.6, 11.88, -22.44],
    // MZ2_SOLDIER_MACHINEGUN_5        91
    [36.6, 11.88, -22.44],
    // MZ2_SOLDIER_BLASTER_6            92
    [33.12, 4.08, -12.48],
    // MZ2_SOLDIER_SHOTGUN_6            93
    [33.12, 4.08, -12.48],
    // MZ2_SOLDIER_MACHINEGUN_6        94
    [33.12, 4.08, -12.48],
    // MZ2_SOLDIER_BLASTER_7            95
    [34.68, 5.52, -9.72],
    // MZ2_SOLDIER_SHOTGUN_7            96
    [34.68, 5.52, -9.72],
    // MZ2_SOLDIER_MACHINEGUN_7        97
    [34.68, 5.52, -9.72],
    // MZ2_SOLDIER_BLASTER_8            98
    // Original was [34.5 * 1.2, 9.6 * 1.2, 6.1 * 1.2] but corrected to [31.5 * 1.2, 9.6 * 1.2, 10.1 * 1.2]
    [37.8, 11.52, 12.12],
    // MZ2_SOLDIER_SHOTGUN_8            99
    [41.4, 11.52, 7.32],
    // MZ2_SOLDIER_MACHINEGUN_8        100
    [41.4, 11.52, 7.32],

    // --- Xian shit below ---
    // MZ2_MAKRON_BFG                    101
    [17.0, -19.5, 62.9],
    // MZ2_MAKRON_BLASTER_1            102
    [-3.6, -24.1, 59.5],
    // MZ2_MAKRON_BLASTER_2            103
    [-1.6, -19.3, 59.5],
    // MZ2_MAKRON_BLASTER_3            104
    [-0.1, -14.4, 59.5],
    // MZ2_MAKRON_BLASTER_4            105
    [2.0, -7.6, 59.5],
    // MZ2_MAKRON_BLASTER_5            106
    [3.4, 1.3, 59.5],
    // MZ2_MAKRON_BLASTER_6            107
    [3.7, 11.1, 59.5],
    // MZ2_MAKRON_BLASTER_7            108
    [-0.3, 22.3, 59.5],
    // MZ2_MAKRON_BLASTER_8            109
    [-6.0, 33.0, 59.5],
    // MZ2_MAKRON_BLASTER_9            110
    [-9.3, 36.4, 59.5],
    // MZ2_MAKRON_BLASTER_10            111
    [-7.0, 35.0, 59.5],
    // MZ2_MAKRON_BLASTER_11            112
    [-2.1, 29.0, 59.5],
    // MZ2_MAKRON_BLASTER_12            113
    [3.9, 17.3, 59.5],
    // MZ2_MAKRON_BLASTER_13            114
    [6.1, 5.8, 59.5],
    // MZ2_MAKRON_BLASTER_14            115
    [5.9, -4.4, 59.5],
    // MZ2_MAKRON_BLASTER_15            116
    [4.2, -14.1, 59.5],
    // MZ2_MAKRON_BLASTER_16            117
    [2.4, -18.8, 59.5],
    // MZ2_MAKRON_BLASTER_17            118
    [-1.8, -25.5, 59.5],
    // MZ2_MAKRON_RAILGUN_1            119
    [-17.3, 7.8, 72.4],

    // MZ2_JORG_MACHINEGUN_L1            120
    [78.5, -47.1, 96.0],
    // MZ2_JORG_MACHINEGUN_L2            121
    [78.5, -47.1, 96.0],
    // MZ2_JORG_MACHINEGUN_L3            122
    [78.5, -47.1, 96.0],
    // MZ2_JORG_MACHINEGUN_L4            123
    [78.5, -47.1, 96.0],
    // MZ2_JORG_MACHINEGUN_L5            124
    [78.5, -47.1, 96.0],
    // MZ2_JORG_MACHINEGUN_L6            125
    [78.5, -47.1, 96.0],
    // MZ2_JORG_MACHINEGUN_R1            126
    [78.5, 46.7, 96.0],
    // MZ2_JORG_MACHINEGUN_R2            127
    [78.5, 46.7, 96.0],
    // MZ2_JORG_MACHINEGUN_R3            128
    [78.5, 46.7, 96.0],
    // MZ2_JORG_MACHINEGUN_R4            129
    [78.5, 46.7, 96.0],
    // MZ2_JORG_MACHINEGUN_R5            130
    [78.5, 46.7, 96.0],
    // MZ2_JORG_MACHINEGUN_R6            131
    [78.5, 46.7, 96.0],
    // MZ2_JORG_BFG_1                    132
    [6.3, -9.0, 111.2],

    // MZ2_BOSS2_MACHINEGUN_R1            133
    [32.0, 40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_R2            134
    [32.0, 40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_R3            135
    [32.0, 40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_R4            136
    [32.0, 40.0, 70.0],
    // MZ2_BOSS2_MACHINEGUN_R5            137
    [32.0, 40.0, 70.0],

    // --- End Xian Shit ---

    // ROGUE
    // note that the above really ends at 137
    // carrier machineguns
    // MZ2_CARRIER_MACHINEGUN_L1        138
    [56.0, -32.0, 32.0],
    // MZ2_CARRIER_MACHINEGUN_R1        139
    [56.0, 32.0, 32.0],
    // MZ2_CARRIER_GRENADE                140
    [42.0, 24.0, 50.0],
    // MZ2_TURRET_MACHINEGUN            141
    [16.0, 0.0, 0.0],
    // MZ2_TURRET_ROCKET                142
    [16.0, 0.0, 0.0],
    // MZ2_TURRET_BLASTER                143
    [16.0, 0.0, 0.0],
    // MZ2_STALKER_BLASTER                144
    [24.0, 0.0, 6.0],
    // MZ2_DAEDALUS_BLASTER            145
    [32.5, -0.8, 10.0],
    // MZ2_MEDIC_BLASTER_2                146
    [12.1, 5.4, 16.5],
    // MZ2_CARRIER_RAILGUN                147
    [32.0, 0.0, 6.0],
    // MZ2_WIDOW_DISRUPTOR                148
    [57.72, 14.50, 88.81],
    // MZ2_WIDOW_BLASTER                149
    [56.0, 32.0, 32.0],
    // MZ2_WIDOW_RAIL                    150
    [62.0, -20.0, 84.0],
    // MZ2_WIDOW_PLASMABEAM            151 (PMM - not used!)
    [32.0, 0.0, 6.0],
    // MZ2_CARRIER_MACHINEGUN_L2        152
    [61.0, -32.0, 12.0],
    // MZ2_CARRIER_MACHINEGUN_R2        153
    [61.0, 32.0, 12.0],
    // MZ2_WIDOW_RAIL_LEFT                154
    [17.0, -62.0, 91.0],
    // MZ2_WIDOW_RAIL_RIGHT            155
    [68.0, 12.0, 86.0],
    // MZ2_WIDOW_BLASTER_SWEEP1        156 (pmm - the sweeps need to be in sequential order)
    [47.5, 56.0, 89.0],
    // MZ2_WIDOW_BLASTER_SWEEP2        157
    [54.0, 52.0, 91.0],
    // MZ2_WIDOW_BLASTER_SWEEP3        158
    [58.0, 40.0, 91.0],
    // MZ2_WIDOW_BLASTER_SWEEP4        159
    [68.0, 30.0, 88.0],
    // MZ2_WIDOW_BLASTER_SWEEP5        160
    [74.0, 20.0, 88.0],
    // MZ2_WIDOW_BLASTER_SWEEP6        161
    [73.0, 11.0, 87.0],
    // MZ2_WIDOW_BLASTER_SWEEP7        162
    [73.0, 3.0, 87.0],
    // MZ2_WIDOW_BLASTER_SWEEP8        163
    [70.0, -12.0, 87.0],
    // MZ2_WIDOW_BLASTER_SWEEP9        164
    [67.0, -20.0, 90.0],
    // MZ2_WIDOW_BLASTER_100            165
    [-20.0, 76.0, 90.0],
    // MZ2_WIDOW_BLASTER_90            166
    [-8.0, 74.0, 90.0],
    // MZ2_WIDOW_BLASTER_80            167
    [0.0, 72.0, 90.0],
    // MZ2_WIDOW_BLASTER_70            168 (d06)
    [10.0, 71.0, 89.0],
    // MZ2_WIDOW_BLASTER_60            169 (d07)
    [23.0, 70.0, 87.0],
    // MZ2_WIDOW_BLASTER_50            170 (d08)
    [32.0, 64.0, 85.0],
    // MZ2_WIDOW_BLASTER_40            171
    [40.0, 58.0, 84.0],
    // MZ2_WIDOW_BLASTER_30            172 (d10)
    [48.0, 50.0, 83.0],
    // MZ2_WIDOW_BLASTER_20            173
    [54.0, 42.0, 82.0],
    // MZ2_WIDOW_BLASTER_10            174 (d12)
    [56.0, 34.0, 82.0],
    // MZ2_WIDOW_BLASTER_0            175
    [58.0, 26.0, 82.0],
    // MZ2_WIDOW_BLASTER_10L            176 (d14)
    [60.0, 16.0, 82.0],
    // MZ2_WIDOW_BLASTER_20L            177
    [59.0, 6.0, 81.0],
    // MZ2_WIDOW_BLASTER_30L            178 (d16)
    [58.0, -2.0, 80.0],
    // MZ2_WIDOW_BLASTER_40L            179
    [57.0, -10.0, 79.0],
    // MZ2_WIDOW_BLASTER_50L            180 (d18)
    [54.0, -18.0, 78.0],
    // MZ2_WIDOW_BLASTER_60L            181
    [42.0, -32.0, 80.0],
    // MZ2_WIDOW_BLASTER_70L            182 (d20)
    [36.0, -40.0, 78.0],
    // MZ2_WIDOW_RUN_1                    183
    [68.4, 10.88, 82.08],
    // MZ2_WIDOW_RUN_2                    184
    [68.51, 8.64, 85.14],
    // MZ2_WIDOW_RUN_3                    185
    [68.66, 6.38, 88.78],
    // MZ2_WIDOW_RUN_4                    186
    [68.73, 5.1, 84.47],
    // MZ2_WIDOW_RUN_5                    187
    [68.82, 4.79, 80.52],
    // MZ2_WIDOW_RUN_6                    188
    [68.77, 6.11, 85.37],
    // MZ2_WIDOW_RUN_7                    189
    [68.67, 7.99, 90.24],
    // MZ2_WIDOW_RUN_8                    190
    [68.55, 9.54, 87.36],
    // MZ2_CARRIER_ROCKET_1            191
    [0.0, 0.0, -5.0],
    // MZ2_CARRIER_ROCKET_2            192
    [0.0, 0.0, -5.0],
    // MZ2_CARRIER_ROCKET_3            193
    [0.0, 0.0, -5.0],
    // MZ2_CARRIER_ROCKET_4            194
    [0.0, 0.0, -5.0],
    // MZ2_WIDOW2_BEAMER_1                195
    // Original was [72.13, -17.63, 93.77] but corrected to [69.00, -17.63, 93.77]
    [69.00, -17.63, 93.77],
    // MZ2_WIDOW2_BEAMER_2                196
    // Original was [71.46, -17.08, 89.82] but corrected to [69.00, -17.08, 89.82]
    [69.00, -17.08, 89.82],
    // MZ2_WIDOW2_BEAMER_3                197
    // Original was [71.47, -18.40, 90.70] but corrected to [69.00, -18.40, 90.70]
    [69.00, -18.40, 90.70],
    // MZ2_WIDOW2_BEAMER_4                198
    // Original was [71.96, -18.34, 94.32] but corrected to [69.00, -18.34, 94.32]
    [69.00, -18.34, 94.32],
    // MZ2_WIDOW2_BEAMER_5                199
    // Original was [72.25, -18.30, 97.98] but corrected to [69.00, -18.30, 97.98]
    [69.00, -18.30, 97.98],
    // MZ2_WIDOW2_BEAM_SWEEP_1        200
    [45.04, -59.02, 92.24],
    // MZ2_WIDOW2_BEAM_SWEEP_2        201
    [50.68, -54.70, 91.96],
    // MZ2_WIDOW2_BEAM_SWEEP_3        202
    [56.57, -47.72, 91.65],
    // MZ2_WIDOW2_BEAM_SWEEP_4        203
    [61.75, -38.75, 91.38],
    // MZ2_WIDOW2_BEAM_SWEEP_5        204
    [65.55, -28.76, 91.24],
    // MZ2_WIDOW2_BEAM_SWEEP_6        205
    [67.79, -18.90, 91.22],
    // MZ2_WIDOW2_BEAM_SWEEP_7        206
    [68.60, -9.52, 91.23],
    // MZ2_WIDOW2_BEAM_SWEEP_8        207
    [68.08, 0.18, 91.32],
    // MZ2_WIDOW2_BEAM_SWEEP_9        208
    [66.14, 9.79, 91.44],
    // MZ2_WIDOW2_BEAM_SWEEP_10        209
    [62.77, 18.91, 91.65],
    // MZ2_WIDOW2_BEAM_SWEEP_11        210
    [58.29, 27.11, 92.00],

    // end of table
    [0.0, 0.0, 0.0],
];
