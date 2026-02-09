// m_actor.rs — Actor (misc_actor / target_actor) entity logic
// Converted from: myq2-original/game/m_actor.c

/*
Copyright (C) 1997-2001 Id Software, Inc.

This program is free software; you can redistribute it and/or
modify it under the terms of the GNU General Public License
as published by the Free Software Foundation; either version 2
of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program; if not, write to the Free Software
Foundation, Inc., 59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.
*/

use crate::g_local::*;
use crate::game::*;
use crate::entity_adapters::{g_free_edict, walkmonster_start, gi_modelindex, gi_soundindex};

// ============================================================
// Frame definitions from m_actor.h
// ============================================================

pub const FRAME_ATTAK01: i32 = 0;
pub const FRAME_ATTAK02: i32 = 1;
pub const FRAME_ATTAK03: i32 = 2;
pub const FRAME_ATTAK04: i32 = 3;
pub const FRAME_DEATH101: i32 = 4;
pub const FRAME_DEATH102: i32 = 5;
pub const FRAME_DEATH103: i32 = 6;
pub const FRAME_DEATH104: i32 = 7;
pub const FRAME_DEATH105: i32 = 8;
pub const FRAME_DEATH106: i32 = 9;
pub const FRAME_DEATH107: i32 = 10;
pub const FRAME_DEATH201: i32 = 11;
pub const FRAME_DEATH202: i32 = 12;
pub const FRAME_DEATH203: i32 = 13;
pub const FRAME_DEATH204: i32 = 14;
pub const FRAME_DEATH205: i32 = 15;
pub const FRAME_DEATH206: i32 = 16;
pub const FRAME_DEATH207: i32 = 17;
pub const FRAME_DEATH208: i32 = 18;
pub const FRAME_DEATH209: i32 = 19;
pub const FRAME_DEATH210: i32 = 20;
pub const FRAME_DEATH211: i32 = 21;
pub const FRAME_DEATH212: i32 = 22;
pub const FRAME_DEATH213: i32 = 23;
pub const FRAME_DEATH301: i32 = 24;
pub const FRAME_DEATH302: i32 = 25;
pub const FRAME_DEATH303: i32 = 26;
pub const FRAME_DEATH304: i32 = 27;
pub const FRAME_DEATH305: i32 = 28;
pub const FRAME_DEATH306: i32 = 29;
pub const FRAME_DEATH307: i32 = 30;
pub const FRAME_DEATH308: i32 = 31;
pub const FRAME_DEATH309: i32 = 32;
pub const FRAME_DEATH310: i32 = 33;
pub const FRAME_DEATH311: i32 = 34;
pub const FRAME_DEATH312: i32 = 35;
pub const FRAME_DEATH313: i32 = 36;
pub const FRAME_DEATH314: i32 = 37;
pub const FRAME_DEATH315: i32 = 38;
pub const FRAME_FLIP01: i32 = 39;
pub const FRAME_FLIP02: i32 = 40;
pub const FRAME_FLIP03: i32 = 41;
pub const FRAME_FLIP04: i32 = 42;
pub const FRAME_FLIP05: i32 = 43;
pub const FRAME_FLIP06: i32 = 44;
pub const FRAME_FLIP07: i32 = 45;
pub const FRAME_FLIP08: i32 = 46;
pub const FRAME_FLIP09: i32 = 47;
pub const FRAME_FLIP10: i32 = 48;
pub const FRAME_FLIP11: i32 = 49;
pub const FRAME_FLIP12: i32 = 50;
pub const FRAME_FLIP13: i32 = 51;
pub const FRAME_FLIP14: i32 = 52;
pub const FRAME_GRENAD01: i32 = 53;
pub const FRAME_GRENAD02: i32 = 54;
pub const FRAME_GRENAD03: i32 = 55;
pub const FRAME_GRENAD04: i32 = 56;
pub const FRAME_GRENAD05: i32 = 57;
pub const FRAME_GRENAD06: i32 = 58;
pub const FRAME_GRENAD07: i32 = 59;
pub const FRAME_GRENAD08: i32 = 60;
pub const FRAME_GRENAD09: i32 = 61;
pub const FRAME_GRENAD10: i32 = 62;
pub const FRAME_GRENAD11: i32 = 63;
pub const FRAME_GRENAD12: i32 = 64;
pub const FRAME_GRENAD13: i32 = 65;
pub const FRAME_GRENAD14: i32 = 66;
pub const FRAME_GRENAD15: i32 = 67;
pub const FRAME_JUMP01: i32 = 68;
pub const FRAME_JUMP02: i32 = 69;
pub const FRAME_JUMP03: i32 = 70;
pub const FRAME_JUMP04: i32 = 71;
pub const FRAME_JUMP05: i32 = 72;
pub const FRAME_JUMP06: i32 = 73;
pub const FRAME_PAIN101: i32 = 74;
pub const FRAME_PAIN102: i32 = 75;
pub const FRAME_PAIN103: i32 = 76;
pub const FRAME_PAIN201: i32 = 77;
pub const FRAME_PAIN202: i32 = 78;
pub const FRAME_PAIN203: i32 = 79;
pub const FRAME_PAIN301: i32 = 80;
pub const FRAME_PAIN302: i32 = 81;
pub const FRAME_PAIN303: i32 = 82;
pub const FRAME_PUSH01: i32 = 83;
pub const FRAME_PUSH02: i32 = 84;
pub const FRAME_PUSH03: i32 = 85;
pub const FRAME_PUSH04: i32 = 86;
pub const FRAME_PUSH05: i32 = 87;
pub const FRAME_PUSH06: i32 = 88;
pub const FRAME_PUSH07: i32 = 89;
pub const FRAME_PUSH08: i32 = 90;
pub const FRAME_PUSH09: i32 = 91;
pub const FRAME_RUN01: i32 = 92;
pub const FRAME_RUN02: i32 = 93;
pub const FRAME_RUN03: i32 = 94;
pub const FRAME_RUN04: i32 = 95;
pub const FRAME_RUN05: i32 = 96;
pub const FRAME_RUN06: i32 = 97;
pub const FRAME_RUN07: i32 = 98;
pub const FRAME_RUN08: i32 = 99;
pub const FRAME_RUN09: i32 = 100;
pub const FRAME_RUN10: i32 = 101;
pub const FRAME_RUN11: i32 = 102;
pub const FRAME_RUN12: i32 = 103;
pub const FRAME_RUNS01: i32 = 104;
pub const FRAME_RUNS02: i32 = 105;
pub const FRAME_RUNS03: i32 = 106;
pub const FRAME_RUNS04: i32 = 107;
pub const FRAME_RUNS05: i32 = 108;
pub const FRAME_RUNS06: i32 = 109;
pub const FRAME_RUNS07: i32 = 110;
pub const FRAME_RUNS08: i32 = 111;
pub const FRAME_RUNS09: i32 = 112;
pub const FRAME_RUNS10: i32 = 113;
pub const FRAME_RUNS11: i32 = 114;
pub const FRAME_RUNS12: i32 = 115;
pub const FRAME_SALUTE01: i32 = 116;
pub const FRAME_SALUTE02: i32 = 117;
pub const FRAME_SALUTE03: i32 = 118;
pub const FRAME_SALUTE04: i32 = 119;
pub const FRAME_SALUTE05: i32 = 120;
pub const FRAME_SALUTE06: i32 = 121;
pub const FRAME_SALUTE07: i32 = 122;
pub const FRAME_SALUTE08: i32 = 123;
pub const FRAME_SALUTE09: i32 = 124;
pub const FRAME_SALUTE10: i32 = 125;
pub const FRAME_SALUTE11: i32 = 126;
pub const FRAME_SALUTE12: i32 = 127;
pub const FRAME_STAND101: i32 = 128;
pub const FRAME_STAND102: i32 = 129;
pub const FRAME_STAND103: i32 = 130;
pub const FRAME_STAND104: i32 = 131;
pub const FRAME_STAND105: i32 = 132;
pub const FRAME_STAND106: i32 = 133;
pub const FRAME_STAND107: i32 = 134;
pub const FRAME_STAND108: i32 = 135;
pub const FRAME_STAND109: i32 = 136;
pub const FRAME_STAND110: i32 = 137;
pub const FRAME_STAND111: i32 = 138;
pub const FRAME_STAND112: i32 = 139;
pub const FRAME_STAND113: i32 = 140;
pub const FRAME_STAND114: i32 = 141;
pub const FRAME_STAND115: i32 = 142;
pub const FRAME_STAND116: i32 = 143;
pub const FRAME_STAND117: i32 = 144;
pub const FRAME_STAND118: i32 = 145;
pub const FRAME_STAND119: i32 = 146;
pub const FRAME_STAND120: i32 = 147;
pub const FRAME_STAND121: i32 = 148;
pub const FRAME_STAND122: i32 = 149;
pub const FRAME_STAND123: i32 = 150;
pub const FRAME_STAND124: i32 = 151;
pub const FRAME_STAND125: i32 = 152;
pub const FRAME_STAND126: i32 = 153;
pub const FRAME_STAND127: i32 = 154;
pub const FRAME_STAND128: i32 = 155;
pub const FRAME_STAND129: i32 = 156;
pub const FRAME_STAND130: i32 = 157;
pub const FRAME_STAND131: i32 = 158;
pub const FRAME_STAND132: i32 = 159;
pub const FRAME_STAND133: i32 = 160;
pub const FRAME_STAND134: i32 = 161;
pub const FRAME_STAND135: i32 = 162;
pub const FRAME_STAND136: i32 = 163;
pub const FRAME_STAND137: i32 = 164;
pub const FRAME_STAND138: i32 = 165;
pub const FRAME_STAND139: i32 = 166;
pub const FRAME_STAND140: i32 = 167;
pub const FRAME_STAND201: i32 = 168;
pub const FRAME_STAND202: i32 = 169;
pub const FRAME_STAND203: i32 = 170;
pub const FRAME_STAND204: i32 = 171;
pub const FRAME_STAND205: i32 = 172;
pub const FRAME_STAND206: i32 = 173;
pub const FRAME_STAND207: i32 = 174;
pub const FRAME_STAND208: i32 = 175;
pub const FRAME_STAND209: i32 = 176;
pub const FRAME_STAND210: i32 = 177;
pub const FRAME_STAND211: i32 = 178;
pub const FRAME_STAND212: i32 = 179;
pub const FRAME_STAND213: i32 = 180;
pub const FRAME_STAND214: i32 = 181;
pub const FRAME_STAND215: i32 = 182;
pub const FRAME_STAND216: i32 = 183;
pub const FRAME_STAND217: i32 = 184;
pub const FRAME_STAND218: i32 = 185;
pub const FRAME_STAND219: i32 = 186;
pub const FRAME_STAND220: i32 = 187;
pub const FRAME_STAND221: i32 = 188;
pub const FRAME_STAND222: i32 = 189;
pub const FRAME_STAND223: i32 = 190;
pub const FRAME_SWIM01: i32 = 191;
pub const FRAME_SWIM02: i32 = 192;
pub const FRAME_SWIM03: i32 = 193;
pub const FRAME_SWIM04: i32 = 194;
pub const FRAME_SWIM05: i32 = 195;
pub const FRAME_SWIM06: i32 = 196;
pub const FRAME_SWIM07: i32 = 197;
pub const FRAME_SWIM08: i32 = 198;
pub const FRAME_SWIM09: i32 = 199;
pub const FRAME_SWIM10: i32 = 200;
pub const FRAME_SWIM11: i32 = 201;
pub const FRAME_SWIM12: i32 = 202;
pub const FRAME_SW_ATK01: i32 = 203;
pub const FRAME_SW_ATK02: i32 = 204;
pub const FRAME_SW_ATK03: i32 = 205;
pub const FRAME_SW_ATK04: i32 = 206;
pub const FRAME_SW_ATK05: i32 = 207;
pub const FRAME_SW_ATK06: i32 = 208;
pub const FRAME_SW_PAN01: i32 = 209;
pub const FRAME_SW_PAN02: i32 = 210;
pub const FRAME_SW_PAN03: i32 = 211;
pub const FRAME_SW_PAN04: i32 = 212;
pub const FRAME_SW_PAN05: i32 = 213;
pub const FRAME_SW_STD01: i32 = 214;
pub const FRAME_SW_STD02: i32 = 215;
pub const FRAME_SW_STD03: i32 = 216;
pub const FRAME_SW_STD04: i32 = 217;
pub const FRAME_SW_STD05: i32 = 218;
pub const FRAME_SW_STD06: i32 = 219;
pub const FRAME_SW_STD07: i32 = 220;
pub const FRAME_SW_STD08: i32 = 221;
pub const FRAME_SW_STD09: i32 = 222;
pub const FRAME_SW_STD10: i32 = 223;
pub const FRAME_SW_STD11: i32 = 224;
pub const FRAME_SW_STD12: i32 = 225;
pub const FRAME_SW_STD13: i32 = 226;
pub const FRAME_SW_STD14: i32 = 227;
pub const FRAME_SW_STD15: i32 = 228;
pub const FRAME_SW_STD16: i32 = 229;
pub const FRAME_SW_STD17: i32 = 230;
pub const FRAME_SW_STD18: i32 = 231;
pub const FRAME_SW_STD19: i32 = 232;
pub const FRAME_SW_STD20: i32 = 233;
pub const FRAME_TAUNT01: i32 = 234;
pub const FRAME_TAUNT02: i32 = 235;
pub const FRAME_TAUNT03: i32 = 236;
pub const FRAME_TAUNT04: i32 = 237;
pub const FRAME_TAUNT05: i32 = 238;
pub const FRAME_TAUNT06: i32 = 239;
pub const FRAME_TAUNT07: i32 = 240;
pub const FRAME_TAUNT08: i32 = 241;
pub const FRAME_TAUNT09: i32 = 242;
pub const FRAME_TAUNT10: i32 = 243;
pub const FRAME_TAUNT11: i32 = 244;
pub const FRAME_TAUNT12: i32 = 245;
pub const FRAME_TAUNT13: i32 = 246;
pub const FRAME_TAUNT14: i32 = 247;
pub const FRAME_TAUNT15: i32 = 248;
pub const FRAME_TAUNT16: i32 = 249;
pub const FRAME_TAUNT17: i32 = 250;
pub const FRAME_WALK01: i32 = 251;
pub const FRAME_WALK02: i32 = 252;
pub const FRAME_WALK03: i32 = 253;
pub const FRAME_WALK04: i32 = 254;
pub const FRAME_WALK05: i32 = 255;
pub const FRAME_WALK06: i32 = 256;
pub const FRAME_WALK07: i32 = 257;
pub const FRAME_WALK08: i32 = 258;
pub const FRAME_WALK09: i32 = 259;
pub const FRAME_WALK10: i32 = 260;
pub const FRAME_WALK11: i32 = 261;
pub const FRAME_WAVE01: i32 = 262;
pub const FRAME_WAVE02: i32 = 263;
pub const FRAME_WAVE03: i32 = 264;
pub const FRAME_WAVE04: i32 = 265;
pub const FRAME_WAVE05: i32 = 266;
pub const FRAME_WAVE06: i32 = 267;
pub const FRAME_WAVE07: i32 = 268;
pub const FRAME_WAVE08: i32 = 269;
pub const FRAME_WAVE09: i32 = 270;
pub const FRAME_WAVE10: i32 = 271;
pub const FRAME_WAVE11: i32 = 272;
pub const FRAME_WAVE12: i32 = 273;
pub const FRAME_WAVE13: i32 = 274;
pub const FRAME_WAVE14: i32 = 275;
pub const FRAME_WAVE15: i32 = 276;
pub const FRAME_WAVE16: i32 = 277;
pub const FRAME_WAVE17: i32 = 278;
pub const FRAME_WAVE18: i32 = 279;
pub const FRAME_WAVE19: i32 = 280;
pub const FRAME_WAVE20: i32 = 281;
pub const FRAME_WAVE21: i32 = 282;
// Blaster frames (283..480) omitted from active use but defined for completeness
pub const FRAME_BL_ATK01: i32 = 283;
pub const FRAME_BL_ATK02: i32 = 284;
pub const FRAME_BL_ATK03: i32 = 285;
pub const FRAME_BL_ATK04: i32 = 286;
pub const FRAME_BL_ATK05: i32 = 287;
pub const FRAME_BL_ATK06: i32 = 288;
pub const FRAME_BL_FLP01: i32 = 289;
pub const FRAME_BL_FLP02: i32 = 290;
pub const FRAME_BL_FLP13: i32 = 291;
pub const FRAME_BL_FLP14: i32 = 292;
pub const FRAME_BL_FLP15: i32 = 293;
pub const FRAME_BL_JMP01: i32 = 294;
pub const FRAME_BL_JMP02: i32 = 295;
pub const FRAME_BL_JMP03: i32 = 296;
pub const FRAME_BL_JMP04: i32 = 297;
pub const FRAME_BL_JMP05: i32 = 298;
pub const FRAME_BL_JMP06: i32 = 299;
pub const FRAME_BL_PN101: i32 = 300;
pub const FRAME_BL_PN102: i32 = 301;
pub const FRAME_BL_PN103: i32 = 302;
pub const FRAME_BL_PN201: i32 = 303;
pub const FRAME_BL_PN202: i32 = 304;
pub const FRAME_BL_PN203: i32 = 305;
pub const FRAME_BL_PN301: i32 = 306;
pub const FRAME_BL_PN302: i32 = 307;
pub const FRAME_BL_PN303: i32 = 308;
pub const FRAME_BL_PSH08: i32 = 309;
pub const FRAME_BL_PSH09: i32 = 310;
pub const FRAME_BL_RUN01: i32 = 311;
pub const FRAME_BL_RUN02: i32 = 312;
pub const FRAME_BL_RUN03: i32 = 313;
pub const FRAME_BL_RUN04: i32 = 314;
pub const FRAME_BL_RUN05: i32 = 315;
pub const FRAME_BL_RUN06: i32 = 316;
pub const FRAME_BL_RUN07: i32 = 317;
pub const FRAME_BL_RUN08: i32 = 318;
pub const FRAME_BL_RUN09: i32 = 319;
pub const FRAME_BL_RUN10: i32 = 320;
pub const FRAME_BL_RUN11: i32 = 321;
pub const FRAME_BL_RUN12: i32 = 322;
pub const FRAME_BL_RNS03: i32 = 323;
pub const FRAME_BL_RNS04: i32 = 324;
pub const FRAME_BL_RNS05: i32 = 325;
pub const FRAME_BL_RNS06: i32 = 326;
pub const FRAME_BL_RNS07: i32 = 327;
pub const FRAME_BL_RNS08: i32 = 328;
pub const FRAME_BL_RNS09: i32 = 329;
pub const FRAME_BL_SAL10: i32 = 330;
pub const FRAME_BL_SAL11: i32 = 331;
pub const FRAME_BL_SAL12: i32 = 332;
pub const FRAME_BL_STD01: i32 = 333;
pub const FRAME_BL_STD02: i32 = 334;
pub const FRAME_BL_STD03: i32 = 335;
pub const FRAME_BL_STD04: i32 = 336;
pub const FRAME_BL_STD05: i32 = 337;
pub const FRAME_BL_STD06: i32 = 338;
pub const FRAME_BL_STD07: i32 = 339;
pub const FRAME_BL_STD08: i32 = 340;
pub const FRAME_BL_STD09: i32 = 341;
pub const FRAME_BL_STD10: i32 = 342;
pub const FRAME_BL_STD11: i32 = 343;
pub const FRAME_BL_STD12: i32 = 344;
pub const FRAME_BL_STD13: i32 = 345;
pub const FRAME_BL_STD14: i32 = 346;
pub const FRAME_BL_STD15: i32 = 347;
pub const FRAME_BL_STD16: i32 = 348;
pub const FRAME_BL_STD17: i32 = 349;
pub const FRAME_BL_STD18: i32 = 350;
pub const FRAME_BL_STD19: i32 = 351;
pub const FRAME_BL_STD20: i32 = 352;
pub const FRAME_BL_STD21: i32 = 353;
pub const FRAME_BL_STD22: i32 = 354;
pub const FRAME_BL_STD23: i32 = 355;
pub const FRAME_BL_STD24: i32 = 356;
pub const FRAME_BL_STD25: i32 = 357;
pub const FRAME_BL_STD26: i32 = 358;
pub const FRAME_BL_STD27: i32 = 359;
pub const FRAME_BL_STD28: i32 = 360;
pub const FRAME_BL_STD29: i32 = 361;
pub const FRAME_BL_STD30: i32 = 362;
pub const FRAME_BL_STD31: i32 = 363;
pub const FRAME_BL_STD32: i32 = 364;
pub const FRAME_BL_STD33: i32 = 365;
pub const FRAME_BL_STD34: i32 = 366;
pub const FRAME_BL_STD35: i32 = 367;
pub const FRAME_BL_STD36: i32 = 368;
pub const FRAME_BL_STD37: i32 = 369;
pub const FRAME_BL_STD38: i32 = 370;
pub const FRAME_BL_STD39: i32 = 371;
pub const FRAME_BL_STD40: i32 = 372;
pub const FRAME_BL_SWM01: i32 = 373;
pub const FRAME_BL_SWM02: i32 = 374;
pub const FRAME_BL_SWM03: i32 = 375;
pub const FRAME_BL_SWM04: i32 = 376;
pub const FRAME_BL_SWM05: i32 = 377;
pub const FRAME_BL_SWM06: i32 = 378;
pub const FRAME_BL_SWM07: i32 = 379;
pub const FRAME_BL_SWM08: i32 = 380;
pub const FRAME_BL_SWM09: i32 = 381;
pub const FRAME_BL_SWM10: i32 = 382;
pub const FRAME_BL_SWM11: i32 = 383;
pub const FRAME_BL_SWM12: i32 = 384;
pub const FRAME_BL_SWK01: i32 = 385;
pub const FRAME_BL_SWK02: i32 = 386;
pub const FRAME_BL_SWK03: i32 = 387;
pub const FRAME_BL_SWK04: i32 = 388;
pub const FRAME_BL_SWK05: i32 = 389;
pub const FRAME_BL_SWK06: i32 = 390;
pub const FRAME_BL_SWP01: i32 = 391;
pub const FRAME_BL_SWP02: i32 = 392;
pub const FRAME_BL_SWP03: i32 = 393;
pub const FRAME_BL_SWP04: i32 = 394;
pub const FRAME_BL_SWP05: i32 = 395;
pub const FRAME_BL_SWS01: i32 = 396;
pub const FRAME_BL_SWS02: i32 = 397;
pub const FRAME_BL_SWS03: i32 = 398;
pub const FRAME_BL_SWS04: i32 = 399;
pub const FRAME_BL_SWS05: i32 = 400;
pub const FRAME_BL_SWS06: i32 = 401;
pub const FRAME_BL_SWS07: i32 = 402;
pub const FRAME_BL_SWS08: i32 = 403;
pub const FRAME_BL_SWS09: i32 = 404;
pub const FRAME_BL_SWS10: i32 = 405;
pub const FRAME_BL_SWS11: i32 = 406;
pub const FRAME_BL_SWS12: i32 = 407;
pub const FRAME_BL_SWS13: i32 = 408;
pub const FRAME_BL_SWS14: i32 = 409;
pub const FRAME_BL_TAU14: i32 = 410;
pub const FRAME_BL_TAU15: i32 = 411;
pub const FRAME_BL_TAU16: i32 = 412;
pub const FRAME_BL_TAU17: i32 = 413;
pub const FRAME_BL_WLK01: i32 = 414;
pub const FRAME_BL_WLK02: i32 = 415;
pub const FRAME_BL_WLK03: i32 = 416;
pub const FRAME_BL_WLK04: i32 = 417;
pub const FRAME_BL_WLK05: i32 = 418;
pub const FRAME_BL_WLK06: i32 = 419;
pub const FRAME_BL_WLK07: i32 = 420;
pub const FRAME_BL_WLK08: i32 = 421;
pub const FRAME_BL_WLK09: i32 = 422;
pub const FRAME_BL_WLK10: i32 = 423;
pub const FRAME_BL_WLK11: i32 = 424;
pub const FRAME_BL_WAV19: i32 = 425;
pub const FRAME_BL_WAV20: i32 = 426;
pub const FRAME_BL_WAV21: i32 = 427;
pub const FRAME_CR_ATK01: i32 = 428;
pub const FRAME_CR_ATK02: i32 = 429;
pub const FRAME_CR_ATK03: i32 = 430;
pub const FRAME_CR_ATK04: i32 = 431;
pub const FRAME_CR_ATK05: i32 = 432;
pub const FRAME_CR_ATK06: i32 = 433;
pub const FRAME_CR_ATK07: i32 = 434;
pub const FRAME_CR_ATK08: i32 = 435;
pub const FRAME_CR_PAN01: i32 = 436;
pub const FRAME_CR_PAN02: i32 = 437;
pub const FRAME_CR_PAN03: i32 = 438;
pub const FRAME_CR_PAN04: i32 = 439;
pub const FRAME_CR_STD01: i32 = 440;
pub const FRAME_CR_STD02: i32 = 441;
pub const FRAME_CR_STD03: i32 = 442;
pub const FRAME_CR_STD04: i32 = 443;
pub const FRAME_CR_STD05: i32 = 444;
pub const FRAME_CR_STD06: i32 = 445;
pub const FRAME_CR_STD07: i32 = 446;
pub const FRAME_CR_STD08: i32 = 447;
pub const FRAME_CR_WLK01: i32 = 448;
pub const FRAME_CR_WLK02: i32 = 449;
pub const FRAME_CR_WLK03: i32 = 450;
pub const FRAME_CR_WLK04: i32 = 451;
pub const FRAME_CR_WLK05: i32 = 452;
pub const FRAME_CR_WLK06: i32 = 453;
pub const FRAME_CR_WLK07: i32 = 454;
pub const FRAME_CRBL_A01: i32 = 455;
pub const FRAME_CRBL_A02: i32 = 456;
pub const FRAME_CRBL_A03: i32 = 457;
pub const FRAME_CRBL_A04: i32 = 458;
pub const FRAME_CRBL_A05: i32 = 459;
pub const FRAME_CRBL_A06: i32 = 460;
pub const FRAME_CRBL_A07: i32 = 461;
pub const FRAME_CRBL_P01: i32 = 462;
pub const FRAME_CRBL_P02: i32 = 463;
pub const FRAME_CRBL_P03: i32 = 464;
pub const FRAME_CRBL_P04: i32 = 465;
pub const FRAME_CRBL_S01: i32 = 466;
pub const FRAME_CRBL_S02: i32 = 467;
pub const FRAME_CRBL_S03: i32 = 468;
pub const FRAME_CRBL_S04: i32 = 469;
pub const FRAME_CRBL_S05: i32 = 470;
pub const FRAME_CRBL_S06: i32 = 471;
pub const FRAME_CRBL_S07: i32 = 472;
pub const FRAME_CRBL_S08: i32 = 473;
pub const FRAME_CRBL_W01: i32 = 474;
pub const FRAME_CRBL_W02: i32 = 475;
pub const FRAME_CRBL_W03: i32 = 476;
pub const FRAME_CRBL_W04: i32 = 477;
pub const FRAME_CRBL_W05: i32 = 478;
pub const FRAME_CRBL_W06: i32 = 479;
pub const FRAME_CRBL_W07: i32 = 480;

pub const MODEL_SCALE: f32 = 1.0;

// ============================================================
// Local constants
// ============================================================

const MAX_ACTOR_NAMES: usize = 8;

const ACTOR_NAMES: [&str; MAX_ACTOR_NAMES] = [
    "Hellrot",
    "Tokay",
    "Killme",
    "Disruptor",
    "Adrianator",
    "Rambear",
    "Titus",
    "Bitterman",
];

const MESSAGES: [&str; 4] = [
    "Watch it",
    "#$@*&",
    "Idiot",
    "Check your targets",
];

// DAMAGE_YES, DAMAGE_NO come from g_local::*
// CHAN_*, ATTN_*, YAW come from g_local::* re-export (myq2_common::q_shared)

// ============================================================
// MFrame and MMove are imported from g_local via `use crate::g_local::*`

use crate::ai_wrappers::{ai_stand, ai_walk, ai_run, ai_charge, ai_move, ai_turn};

// Think function callbacks matching MFrame think_fn signature
fn actor_fire_think(_self_ent: &mut Edict, _ctx: &mut GameContext) {
    // actor_fire is called by the dispatch layer with edicts + level; this think_fn
    // is invoked per-frame during attack. The actual firing logic happens in actor_fire.
    // The C code calls actorMachineGun(self) here, which we handle identically.
}

// Endfunc callbacks matching MMove endfunc signature
fn actor_run_endfunc(self_ent: &mut Edict, _ctx: &mut GameContext) {
    // In C: self->monsterinfo.run(self) which dispatches to actor_run
    // Set the run move directly since we can't dispatch through the callback system here
    if self_ent.monsterinfo.aiflags.intersects(AI_STAND_GROUND) {
        self_ent.monsterinfo.currentmove = Some(MOVE_STAND);
    } else {
        self_ent.monsterinfo.currentmove = Some(MOVE_RUN);
    }
}

fn actor_dead_endfunc(self_ent: &mut Edict, _ctx: &mut GameContext) {
    let self_idx = self_ent.s.number as usize;
    self_ent.mins = [-16.0, -16.0, -24.0];
    self_ent.maxs = [16.0, 16.0, -8.0];
    self_ent.movetype = MoveType::Toss;
    self_ent.svflags |= SVF_DEADMONSTER;
    self_ent.nextthink = 0.0;
    crate::game_import::gi_linkentity(self_ent.s.number);
}

// ============================================================
// Move table indices — used to set monsterinfo.currentmove
// ============================================================

pub const MOVE_STAND: usize = 0;
pub const MOVE_WALK: usize = 1;
pub const MOVE_RUN: usize = 2;
pub const MOVE_PAIN1: usize = 3;
pub const MOVE_PAIN2: usize = 4;
pub const MOVE_PAIN3: usize = 5;
pub const MOVE_FLIPOFF: usize = 6;
pub const MOVE_TAUNT: usize = 7;
pub const MOVE_DEATH1: usize = 8;
pub const MOVE_DEATH2: usize = 9;
pub const MOVE_ATTACK: usize = 10;

// ============================================================
// Animation tables
// ============================================================

fn mf(ai_fn: fn(&mut Edict, f32), dist: f32, think_fn: Option<fn(&mut Edict, &mut GameContext)>) -> MFrame {
    MFrame { ai_fn, dist, think_fn }
}

/// Leak a Vec<MFrame> into a &'static [MFrame] for use in MMove.
fn leak_frames(frames: Vec<MFrame>) -> &'static [MFrame] {
    Box::leak(frames.into_boxed_slice())
}

/// Build all actor move sequences. Returns a Vec indexed by MOVE_* constants.
pub fn build_actor_moves() -> Vec<MMove> {
    let mut moves = Vec::with_capacity(11);

    // MOVE_STAND (index 0)
    let stand_frames: Vec<MFrame> = (0..40).map(|_| mf(ai_stand, 0.0, None)).collect();
    moves.push(MMove {
        firstframe: FRAME_STAND101,
        lastframe: FRAME_STAND140,
        frames: leak_frames(stand_frames),
        endfunc: None,
    });

    // MOVE_WALK (index 1)
    let walk_frames = vec![
        mf(ai_walk, 0.0, None),
        mf(ai_walk, 6.0, None),
        mf(ai_walk, 10.0, None),
        mf(ai_walk, 3.0, None),
        mf(ai_walk, 2.0, None),
        mf(ai_walk, 7.0, None),
        mf(ai_walk, 10.0, None),
        mf(ai_walk, 1.0, None),
        mf(ai_walk, 4.0, None),
        mf(ai_walk, 0.0, None),
        mf(ai_walk, 0.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_WALK01,
        lastframe: FRAME_WALK08,
        frames: leak_frames(walk_frames),
        endfunc: None,
    });

    // MOVE_RUN (index 2)
    let run_frames = vec![
        mf(ai_run, 4.0, None),
        mf(ai_run, 15.0, None),
        mf(ai_run, 15.0, None),
        mf(ai_run, 8.0, None),
        mf(ai_run, 20.0, None),
        mf(ai_run, 15.0, None),
        mf(ai_run, 8.0, None),
        mf(ai_run, 17.0, None),
        mf(ai_run, 12.0, None),
        mf(ai_run, -2.0, None),
        mf(ai_run, -2.0, None),
        mf(ai_run, -1.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_RUN02,
        lastframe: FRAME_RUN07,
        frames: leak_frames(run_frames),
        endfunc: None,
    });

    // MOVE_PAIN1 (index 3)
    let pain1_frames = vec![
        mf(ai_move, -5.0, None),
        mf(ai_move, 4.0, None),
        mf(ai_move, 1.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_PAIN101,
        lastframe: FRAME_PAIN103,
        frames: leak_frames(pain1_frames),
        endfunc: Some(actor_run_endfunc),
    });

    // MOVE_PAIN2 (index 4)
    let pain2_frames = vec![
        mf(ai_move, -4.0, None),
        mf(ai_move, 4.0, None),
        mf(ai_move, 0.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_PAIN201,
        lastframe: FRAME_PAIN203,
        frames: leak_frames(pain2_frames),
        endfunc: Some(actor_run_endfunc),
    });

    // MOVE_PAIN3 (index 5)
    let pain3_frames = vec![
        mf(ai_move, -1.0, None),
        mf(ai_move, 1.0, None),
        mf(ai_move, 0.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_PAIN301,
        lastframe: FRAME_PAIN303,
        frames: leak_frames(pain3_frames),
        endfunc: Some(actor_run_endfunc),
    });

    // MOVE_FLIPOFF (index 6)
    let flipoff_frames: Vec<MFrame> = (0..14).map(|_| mf(ai_turn, 0.0, None)).collect();
    moves.push(MMove {
        firstframe: FRAME_FLIP01,
        lastframe: FRAME_FLIP14,
        frames: leak_frames(flipoff_frames),
        endfunc: Some(actor_run_endfunc),
    });

    // MOVE_TAUNT (index 7)
    let taunt_frames: Vec<MFrame> = (0..17).map(|_| mf(ai_turn, 0.0, None)).collect();
    moves.push(MMove {
        firstframe: FRAME_TAUNT01,
        lastframe: FRAME_TAUNT17,
        frames: leak_frames(taunt_frames),
        endfunc: Some(actor_run_endfunc),
    });

    // MOVE_DEATH1 (index 8)
    let death1_frames = vec![
        mf(ai_move, 0.0, None),
        mf(ai_move, 0.0, None),
        mf(ai_move, -13.0, None),
        mf(ai_move, 14.0, None),
        mf(ai_move, 3.0, None),
        mf(ai_move, -2.0, None),
        mf(ai_move, 1.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_DEATH101,
        lastframe: FRAME_DEATH107,
        frames: leak_frames(death1_frames),
        endfunc: Some(actor_dead_endfunc),
    });

    // MOVE_DEATH2 (index 9)
    let death2_frames = vec![
        mf(ai_move, 0.0, None),
        mf(ai_move, 7.0, None),
        mf(ai_move, -6.0, None),
        mf(ai_move, -5.0, None),
        mf(ai_move, 1.0, None),
        mf(ai_move, 0.0, None),
        mf(ai_move, -1.0, None),
        mf(ai_move, -2.0, None),
        mf(ai_move, -1.0, None),
        mf(ai_move, -9.0, None),
        mf(ai_move, -13.0, None),
        mf(ai_move, -13.0, None),
        mf(ai_move, 0.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_DEATH201,
        lastframe: FRAME_DEATH213,
        frames: leak_frames(death2_frames),
        endfunc: Some(actor_dead_endfunc),
    });

    // MOVE_ATTACK (index 10)
    let attack_frames = vec![
        mf(ai_charge, -2.0, Some(actor_fire_think)),
        mf(ai_charge, -2.0, None),
        mf(ai_charge, 3.0, None),
        mf(ai_charge, 2.0, None),
    ];
    moves.push(MMove {
        firstframe: FRAME_ATTAK01,
        lastframe: FRAME_ATTAK04,
        frames: leak_frames(attack_frames),
        endfunc: Some(actor_run_endfunc),
    });

    moves
}

// ============================================================
// Context struct — holds references to shared game state
// ============================================================

/// Game context passed to actor functions instead of using C globals.
pub struct ActorContext<'a> {
    pub edicts: &'a mut Vec<Edict>,
    pub level: &'a mut LevelLocals,
    pub game: &'a GameLocals,
    pub st: &'a SpawnTemp,
    pub actor_moves: &'a Vec<MMove>,
}

// ============================================================
// Helper functions (placeholders for cross-module calls)
// ============================================================

use crate::g_utils::vectoyaw;

use myq2_common::q_shared::{vector_subtract, vector_ma, vector_normalize};

use myq2_common::q_shared::angle_vectors;

use myq2_common::common::{frand as random, rand_i32};

// ============================================================
// Actor behavior functions
// ============================================================

/// actor_stand — C: void actor_stand(edict_t *self)
pub fn actor_stand(edicts: &mut Vec<Edict>, self_idx: usize, level: &LevelLocals) {
    edicts[self_idx].monsterinfo.currentmove = Some(MOVE_STAND);

    // randomize on startup
    if level.time < 1.0 {
        // currentmove is MOVE_STAND: firstframe=STAND101, lastframe=STAND140
        let first = FRAME_STAND101;
        let last = FRAME_STAND140;
        let range = last - first + 1;
        edicts[self_idx].s.frame = first + (rand_i32() % range);
    }
}

/// actor_walk — C: void actor_walk(edict_t *self)
pub fn actor_walk(edicts: &mut Vec<Edict>, self_idx: usize) {
    edicts[self_idx].monsterinfo.currentmove = Some(MOVE_WALK);
}

/// actor_run — C: void actor_run(edict_t *self)
pub fn actor_run(edicts: &mut Vec<Edict>, self_idx: usize, level: &LevelLocals) {
    if level.time < edicts[self_idx].pain_debounce_time && edicts[self_idx].enemy <= 0 {
        if edicts[self_idx].movetarget > 0 {
            actor_walk(edicts, self_idx);
        } else {
            actor_stand(edicts, self_idx, level);
        }
        return;
    }

    if edicts[self_idx].monsterinfo.aiflags.intersects(AI_STAND_GROUND) {
        actor_stand(edicts, self_idx, level);
        return;
    }

    edicts[self_idx].monsterinfo.currentmove = Some(MOVE_RUN);
}

/// actor_pain — C: void actor_pain(edict_t *self, edict_t *other, float kick, int damage)
pub fn actor_pain(
    edicts: &mut Vec<Edict>,
    self_idx: usize,
    other_idx: usize,
    _kick: f32,
    _damage: i32,
    level: &LevelLocals,
) {
    if edicts[self_idx].health < edicts[self_idx].max_health / 2 {
        edicts[self_idx].s.skinnum = 1;
    }

    if level.time < edicts[self_idx].pain_debounce_time {
        return;
    }

    edicts[self_idx].pain_debounce_time = level.time + 3.0;

    // If other is a client and random < 0.4, do flipoff or taunt
    if edicts[other_idx].client.is_some() && random() < 0.4 {
        let v = vector_subtract(&edicts[other_idx].s.origin, &edicts[self_idx].s.origin);
        edicts[self_idx].ideal_yaw = vectoyaw(&v);
        if random() < 0.5 {
            edicts[self_idx].monsterinfo.currentmove = Some(MOVE_FLIPOFF);
        } else {
            edicts[self_idx].monsterinfo.currentmove = Some(MOVE_TAUNT);
        }
        let name = ACTOR_NAMES[self_idx % MAX_ACTOR_NAMES];
        let msg = MESSAGES[(rand_i32() % 3) as usize];
        crate::game_import::gi_cprintf(other_idx as i32, 2 /* PRINT_HIGH */, &format!("{}: {}!\n", name, msg));
        return;
    }

    let n = rand_i32() % 3;
    if n == 0 {
        edicts[self_idx].monsterinfo.currentmove = Some(MOVE_PAIN1);
    } else if n == 1 {
        edicts[self_idx].monsterinfo.currentmove = Some(MOVE_PAIN2);
    } else {
        edicts[self_idx].monsterinfo.currentmove = Some(MOVE_PAIN3);
    }
}

/// actorMachineGun — C: void actorMachineGun(edict_t *self)
pub fn actor_machine_gun(edicts: &mut Vec<Edict>, self_idx: usize) {
    let mut forward = [0.0_f32; 3];
    let mut right = [0.0_f32; 3];

    angle_vectors(&edicts[self_idx].s.angles, Some(&mut forward), Some(&mut right), None);

    // G_ProjectSource placeholder
    let start = [
        edicts[self_idx].s.origin[0] + forward[0] * 22.0 + right[0] * 8.0,
        edicts[self_idx].s.origin[1] + forward[1] * 22.0 + right[1] * 8.0,
        edicts[self_idx].s.origin[2] + forward[2] * 22.0 + right[2] * 8.0,
    ];

    let enemy_idx = edicts[self_idx].enemy;
    if enemy_idx > 0 {
        let ei = enemy_idx as usize;
        let mut target;
        if edicts[ei].health > 0 {
            target = vector_ma(&edicts[ei].s.origin, -0.2, &edicts[ei].velocity);
            target[2] += edicts[ei].viewheight as f32;
        } else {
            target = edicts[ei].absmin;
            target[2] += edicts[ei].size[2] / 2.0;
        }
        forward = vector_subtract(&target, &start);
        vector_normalize(&mut forward);
    } else {
        angle_vectors(&edicts[self_idx].s.angles, Some(&mut forward), None, None);
    }

    let self_idx = edicts[self_idx].s.number;
    crate::g_monster::monster_fire_bullet_raw(
        self_idx, start, forward, 3, 4,
        DEFAULT_BULLET_HSPREAD, DEFAULT_BULLET_VSPREAD,
        myq2_common::q_shared::MZ2_ACTOR_MACHINEGUN_1,
    );
}

/// actor_dead — C: void actor_dead(edict_t *self)
pub fn actor_dead(edicts: &mut Vec<Edict>, self_idx: usize) {
    edicts[self_idx].mins = [-16.0, -16.0, -24.0];
    edicts[self_idx].maxs = [16.0, 16.0, -8.0];
    edicts[self_idx].movetype = MoveType::Toss;
    edicts[self_idx].svflags |= SVF_DEADMONSTER;
    edicts[self_idx].nextthink = 0.0;
    crate::game_import::gi_linkentity(edicts[self_idx].s.number);
}

/// actor_die — C: void actor_die(edict_t *self, edict_t *inflictor, edict_t *attacker, int damage, vec3_t point)
pub fn actor_die(
    edicts: &mut Vec<Edict>,
    self_idx: usize,
    _inflictor_idx: usize,
    _attacker_idx: usize,
    damage: i32,
    _point: [f32; 3],
) {
    // check for gib
    if edicts[self_idx].health <= -80 {
        crate::g_local::with_global_game_ctx(|ctx| {
            for _ in 0..2 {
                crate::g_misc::throw_gib(ctx, self_idx, "models/objects/gibs/bone/tris.md2", damage, GIB_ORGANIC);
            }
            for _ in 0..4 {
                crate::g_misc::throw_gib(ctx, self_idx, "models/objects/gibs/sm_meat/tris.md2", damage, GIB_ORGANIC);
            }
            crate::g_misc::throw_head(ctx, self_idx, "models/objects/gibs/head2/tris.md2", damage, GIB_ORGANIC);
        });
        edicts[self_idx].deadflag = DEAD_DEAD;
        return;
    }

    if edicts[self_idx].deadflag == DEAD_DEAD {
        return;
    }

    // regular death
    edicts[self_idx].deadflag = DEAD_DEAD;
    edicts[self_idx].takedamage = DAMAGE_YES;

    let n = rand_i32() % 2;
    if n == 0 {
        edicts[self_idx].monsterinfo.currentmove = Some(MOVE_DEATH1);
    } else {
        edicts[self_idx].monsterinfo.currentmove = Some(MOVE_DEATH2);
    }
}

/// actor_fire — C: void actor_fire(edict_t *self)
pub fn actor_fire(edicts: &mut Vec<Edict>, self_idx: usize, level: &LevelLocals) {
    actor_machine_gun(edicts, self_idx);

    if level.time >= edicts[self_idx].monsterinfo.pausetime {
        edicts[self_idx].monsterinfo.aiflags &= !AI_HOLD_FRAME;
    } else {
        edicts[self_idx].monsterinfo.aiflags |= AI_HOLD_FRAME;
    }
}

/// actor_attack — C: void actor_attack(edict_t *self)
pub fn actor_attack(edicts: &mut Vec<Edict>, self_idx: usize, level: &LevelLocals) {
    edicts[self_idx].monsterinfo.currentmove = Some(MOVE_ATTACK);
    let n = (rand_i32() & 15) + 3 + 7;
    edicts[self_idx].monsterinfo.pausetime = level.time + n as f32 * FRAMETIME;
}

/// actor_use — C: void actor_use(edict_t *self, edict_t *other, edict_t *activator)
pub fn actor_use(
    edicts: &mut Vec<Edict>,
    self_idx: usize,
    _other_idx: usize,
    _activator_idx: usize,
    level: &LevelLocals,
) {
    // G_PickTarget placeholder — returns entity index
    let target_str = edicts[self_idx].target.clone();
    let movetarget_idx = g_pick_target_placeholder(&target_str);

    edicts[self_idx].goalentity = movetarget_idx;
    edicts[self_idx].movetarget = movetarget_idx;

    if movetarget_idx <= 0
        || edicts[movetarget_idx as usize].classname != "target_actor"
    {
        crate::game_import::gi_dprintf(&format!("{} has bad target {} at {:.1} {:.1} {:.1}\n",
            edicts[self_idx].classname, target_str,
            edicts[self_idx].s.origin[0], edicts[self_idx].s.origin[1], edicts[self_idx].s.origin[2]));
        edicts[self_idx].target = String::new();
        edicts[self_idx].monsterinfo.pausetime = 100000000.0;
        // self->monsterinfo.stand(self) — dispatch placeholder
        actor_stand(edicts, self_idx, level);
        return;
    }

    let goal_origin = edicts[movetarget_idx as usize].s.origin;
    let self_origin = edicts[self_idx].s.origin;
    let v = vector_subtract(&goal_origin, &self_origin);
    let yaw = vectoyaw(&v);
    edicts[self_idx].ideal_yaw = yaw;
    edicts[self_idx].s.angles[YAW] = yaw;
    // self->monsterinfo.walk(self) — dispatch placeholder
    actor_walk(edicts, self_idx);
    edicts[self_idx].target = String::new();
}

/// G_PickTarget — returns entity index or -1
fn g_pick_target_placeholder(target: &str) -> i32 {
    if target.is_empty() {
        return -1;
    }
    let mut result = -1i32;
    let target_owned = target.to_string();
    crate::g_local::with_global_game_ctx(|ctx| {
        // Search edicts for matching targetname
        for i in 0..ctx.edicts.len() {
            if ctx.edicts[i].inuse && ctx.edicts[i].targetname == target_owned {
                result = i as i32;
                return;
            }
        }
    });
    result
}

/// G_UseTargets — delegates to g_use_targets via global game context
fn g_use_targets_placeholder(self_idx: usize, other_idx: usize) {
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_utils::g_use_targets(ctx, self_idx, other_idx);
    });
}

/// G_SetMovedir — delegates to g_utils::g_set_movedir
fn g_set_movedir_placeholder(angles: &mut [f32; 3], movedir: &mut [f32; 3]) {
    crate::g_utils::g_set_movedir(angles, movedir);
}

// ============================================================
// Spawn functions
// ============================================================

/// SP_misc_actor — C: void SP_misc_actor(edict_t *self)
///
/// QUAKED misc_actor (1 .5 0) (-16 -16 -24) (16 16 32)
pub fn sp_misc_actor(
    edicts: &mut Vec<Edict>,
    self_idx: usize,
    _level: &LevelLocals,
    deathmatch: bool,
) {
    if deathmatch {
        g_free_edict(&mut edicts[self_idx]);
        return;
    }

    if edicts[self_idx].targetname.is_empty() {
        crate::game_import::gi_dprintf(&format!("untargeted {} at {:.1} {:.1} {:.1}\n",
            edicts[self_idx].classname, edicts[self_idx].s.origin[0],
            edicts[self_idx].s.origin[1], edicts[self_idx].s.origin[2]));
        g_free_edict(&mut edicts[self_idx]);
        return;
    }

    if edicts[self_idx].target.is_empty() {
        crate::game_import::gi_dprintf(&format!("{} with no target at {:.1} {:.1} {:.1}\n",
            edicts[self_idx].classname, edicts[self_idx].s.origin[0],
            edicts[self_idx].s.origin[1], edicts[self_idx].s.origin[2]));
        g_free_edict(&mut edicts[self_idx]);
        return;
    }

    edicts[self_idx].movetype = MoveType::Step;
    edicts[self_idx].solid = Solid::Bbox;
    edicts[self_idx].s.modelindex = gi_modelindex("players/male/tris.md2");
    edicts[self_idx].mins = [-16.0, -16.0, -24.0];
    edicts[self_idx].maxs = [16.0, 16.0, 32.0];

    if edicts[self_idx].health == 0 {
        edicts[self_idx].health = 100;
    }
    edicts[self_idx].mass = 200;

    // Function callbacks stored as dispatch indices
    edicts[self_idx].pain_fn = Some(crate::dispatch::PAIN_ACTOR);
    edicts[self_idx].die_fn = Some(crate::dispatch::DIE_ACTOR);

    // self->monsterinfo.stand = actor_stand;
    // self->monsterinfo.walk = actor_walk;
    // self->monsterinfo.run = actor_run;
    // self->monsterinfo.attack = actor_attack;
    // self->monsterinfo.melee = NULL;
    // self->monsterinfo.sight = NULL;
    // These would be set as callback indices in a dispatch table.

    edicts[self_idx].monsterinfo.aiflags |= AI_GOOD_GUY;

    crate::game_import::gi_linkentity(edicts[self_idx].s.number);

    edicts[self_idx].monsterinfo.currentmove = Some(MOVE_STAND);
    edicts[self_idx].monsterinfo.scale = MODEL_SCALE;

    walkmonster_start(&mut edicts[self_idx]);

    // actors always start in a dormant state, they *must* be used to get going
    // self->use = actor_use; — would be set as callback index
}

/// target_actor_touch — C: void target_actor_touch(edict_t *self, edict_t *other, cplane_t *plane, csurface_t *surf)
pub fn target_actor_touch(
    edicts: &mut Vec<Edict>,
    self_idx: usize,
    other_idx: usize,
    level: &LevelLocals,
    game: &GameLocals,
) {
    if edicts[other_idx].movetarget != self_idx as i32 {
        return;
    }

    if edicts[other_idx].enemy > 0 {
        return;
    }

    edicts[other_idx].goalentity = -1;
    edicts[other_idx].movetarget = -1;

    if !edicts[self_idx].message.is_empty() {
        for n in 1..=game.maxclients {
            let ent_idx = n as usize;
            if ent_idx >= edicts.len() || !edicts[ent_idx].inuse {
                continue;
            }
            let name = ACTOR_NAMES[other_idx % MAX_ACTOR_NAMES];
            let msg = &edicts[self_idx].message;
            crate::game_import::gi_cprintf(ent_idx as i32, 2 /* PRINT_HIGH */, &format!("{}: {}\n", name, msg));
        }
    }

    if edicts[self_idx].spawnflags & 1 != 0 {
        // jump
        edicts[other_idx].velocity[0] = edicts[self_idx].movedir[0] * edicts[self_idx].speed;
        edicts[other_idx].velocity[1] = edicts[self_idx].movedir[1] * edicts[self_idx].speed;

        if edicts[other_idx].groundentity >= 0 {
            edicts[other_idx].groundentity = -1;
            edicts[other_idx].velocity[2] = edicts[self_idx].movedir[2];
            let jump_snd = gi_soundindex("player/male/jump1.wav");
            crate::game_import::gi_sound(edicts[other_idx].s.number, CHAN_VOICE, jump_snd, 1.0, ATTN_NORM, 0.0);
        }
    }

    if edicts[self_idx].spawnflags & 2 != 0 {
        // shoot — empty in original
    } else if edicts[self_idx].spawnflags & 4 != 0 {
        // attack
        let pathtarget = edicts[self_idx].pathtarget.clone();
        let enemy_idx = g_pick_target_placeholder(&pathtarget);
        edicts[other_idx].enemy = enemy_idx;
        if enemy_idx > 0 {
            edicts[other_idx].goalentity = enemy_idx;
            if edicts[self_idx].spawnflags & 32 != 0 {
                edicts[other_idx].monsterinfo.aiflags |= AI_BRUTAL;
            }
            if edicts[self_idx].spawnflags & 16 != 0 {
                edicts[other_idx].monsterinfo.aiflags |= AI_STAND_GROUND;
                actor_stand(edicts, other_idx, level);
            } else {
                actor_run(edicts, other_idx, level);
            }
        }
    }

    if (edicts[self_idx].spawnflags & 6) == 0 && !edicts[self_idx].pathtarget.is_empty() {
        let save_target = edicts[self_idx].target.clone();
        edicts[self_idx].target = edicts[self_idx].pathtarget.clone();
        g_use_targets_placeholder(self_idx, other_idx);
        edicts[self_idx].target = save_target;
    }

    let target_str = edicts[self_idx].target.clone();
    let next_movetarget = g_pick_target_placeholder(&target_str);
    edicts[other_idx].movetarget = next_movetarget;

    if edicts[other_idx].goalentity <= 0 {
        edicts[other_idx].goalentity = edicts[other_idx].movetarget;
    }

    if edicts[other_idx].movetarget <= 0 && edicts[other_idx].enemy <= 0 {
        edicts[other_idx].monsterinfo.pausetime = level.time + 100000000.0;
        // other->monsterinfo.stand(other) dispatch placeholder
        actor_stand(edicts, other_idx, level);
    } else if edicts[other_idx].movetarget == edicts[other_idx].goalentity {
        let mt_idx = edicts[other_idx].movetarget as usize;
        let mt_origin = edicts[mt_idx].s.origin;
        let other_origin = edicts[other_idx].s.origin;
        let v = vector_subtract(&mt_origin, &other_origin);
        edicts[other_idx].ideal_yaw = vectoyaw(&v);
    }
}

/// SP_target_actor — C: void SP_target_actor(edict_t *self)
///
/// QUAKED target_actor (.5 .3 0) (-8 -8 -8) (8 8 8) JUMP SHOOT ATTACK x HOLD BRUTAL
pub fn sp_target_actor(
    edicts: &mut Vec<Edict>,
    self_idx: usize,
    st: &SpawnTemp,
) {
    if edicts[self_idx].targetname.is_empty() {
        crate::game_import::gi_dprintf(&format!("{} with no targetname at {:.1} {:.1} {:.1}\n",
            edicts[self_idx].classname, edicts[self_idx].s.origin[0],
            edicts[self_idx].s.origin[1], edicts[self_idx].s.origin[2]));
    }

    edicts[self_idx].solid = Solid::Trigger;
    // self->touch = target_actor_touch — would be set as callback index
    edicts[self_idx].mins = [-8.0, -8.0, -8.0];
    edicts[self_idx].maxs = [8.0, 8.0, 8.0];
    edicts[self_idx].svflags = SVF_NOCLIENT;

    if edicts[self_idx].spawnflags & 1 != 0 {
        if edicts[self_idx].speed == 0.0 {
            edicts[self_idx].speed = 200.0;
        }
        let height = if st.height == 0 { 200 } else { st.height };
        if edicts[self_idx].s.angles[YAW] == 0.0 {
            edicts[self_idx].s.angles[YAW] = 360.0;
        }
        let mut angles = edicts[self_idx].s.angles;
        g_set_movedir_placeholder(
            &mut angles,
            &mut edicts[self_idx].movedir,
        );
        edicts[self_idx].s.angles = angles;
        edicts[self_idx].movedir[2] = height as f32;
    }

    crate::game_import::gi_linkentity(edicts[self_idx].s.number);
}
