// SPDX-License-Identifier: Apache-2.0
//! fs.volume's defects as D13 switches (`specs/freesurfer-volume.md` §69.4, parts 1 to 6c): each one noted in
//! both modes on a case that meets it, and its two modes' results different where the correction
//! changes them (memory reads and crashes give the same result in both modes; the end of the
//! program that §22's modes table gives in both modes differs only by FS-01's NUL in its C-style
//! message). The cases are lines
//! of the comparison's own seeded batches (the shortest line on which the clean room notes the
//! defect in both modes), run through the probe's commands (`tests/volume_cmd/`; the transform
//! files in memory, `ltamem`); FS-292, FS-296, FS-297, FS-302, FS-305, FS-258, FS-323, FS-324, FS-326 and FS-461,
//! which those batches do not meet in both modes, on lines or volumes written here. Also the
//! others' defects these parts meet (FS-12, FS-47, ITK-112, FS-52, FS-53, FS-58).

mod volume_cmd;

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::mri::{Mri, types};
use larmorx_fs_core::volume::Dst;
use larmorx_fs_core::{Environment, Process};

/// (defect, the two modes' results differ, the line).
const CASES: &[(&str, bool, &str)] = &[
    (
        "FS-285",
        false,
        "vgread x76616c6964203d203120202320766f6c756d6520696e666f2076616c69640a78787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878787878203d202f612f6220632e6d677a0a766f6c756d65203d20323639203631203139310a766f78656c73697a65203d20392e333735303030303030303030303030652d303120392e333735303030303030303030303030652d303120312e303030303030303030303030303030652b30300a787261732020203d20382e373131373439313730313535373534652d303120362e333339373135373236323337323633652d303120312e313332323636313032323036303334652d30310a797261732020203d20362e303531323430323831393231383434652d3031202d342e313731303735373637363136373730652d303120382e373835343030343233333632363436652d30310a7a7261732020203d202d372e313236353332353339343631313939652d303120332e353437343533353336383834353836652d3031202d372e383332393836333236393337303333652d30310a 1 1 1 5 3f000000 3f000000 40000000 0 00000000 bf800000 00000000 bf800000 00000000 00000000 00000000 00000000 bf800000 bfab9c3c c2af180a 42820000 00000000 00000000 00000000 x6f7269672e6d677a",
    ),
    (
        "FS-286",
        true,
        "vgread x76616c6964203d203120202320766f6c756d6520696e666f2076616c69640a66696c656e616d65203d206f7269672e6d677a0a766f6c756d65203d2032373520313535203132360a766f78656c73697a65203d7820392e333735303030303030303030303030652d303120312e323530303030303030303030303030652b303020392e333735303030303030303030303030652d30310a787261732020203d202d342e343337303038303131303237313836652d3031202d352e323530383234313736383733323431652d303120392e333337323231393538363039323739652d30310a7a7261732020203d202d312e333637323039353936373232313233652d3031202d362e383433353134323737333239383334652d303120362e353133343631363436373237313534652d30310a637261732020203d202d332e333434343535393535313535383930652b303120342e363039343339383335303130303531652b303120362e323239303935393134383031373839652b30300a 0 6 2 5 3f800000 3f800000 3f800000 1 00000000 3f800000 00000000 bf800000 00000000 00000000 00000000 00000000 3f800000 422841a0 c255fd02 428f3c5e 00000000 00000000 00000000 x",
    ),
    (
        "FS-287",
        false,
        "vgread x76616c6964203d203220202320766f6c756d6520696e666f2076616c69640a66696c656e616d65203d202f612f6220632e6d677a0a090a766f6c756d65203d20323832203932203131340a766f78656c73697a65203d20392e333735303030303030303030303030652d303120312e303030303030303030303030303030652b303020392e333735303030303030303030303030652d30310a787261732020203d20312e363932343932363534363431333232652d3031202d372e333238393334373035303336353734652d3031202d382e393130333030393834393534333936652d30310a797261732020203d202d352e303836333837303638383435303535652d303220342e303031383231333237363235373939652d303120312e363636303435323233373736393236652d30310a7a726173203d3d20203d202d352e373539313932373432363832353430652d3031202d352e373937343737383039393038353837652d303120332e363831303835303330313634343232652d30310a 1 1 3 6 3f800000 3f800000 3f800000 0 3f1fbc43 bf73c5b0 be28b996 3ed14791 bf6bbdd5 bf4a35b7 be1f13ec 3f695714 bf7d5d4d b16a70a9 3f800000 c1880000 00000000 00000000 00000000 x",
    ),
    (
        "FS-288",
        false,
        "vgread x090a76616c6964203d203220202320766f6c756d6520696e666f2076616c69640a66696c656e616d65203d20787878787878787878780a766f6c756d65203d203230203737203235300a766f78656c73697a65203d20352e303030303030303030303030303030652d303120392e333735303030303030303030303030652d303120312e303030303030303030303030303030652b30300a797261732020203d20332e373033303538303439323232313332652d3031202d382e303832313834363637393933353937652d3032202d342e373136353530393032343638373631652d30310a7a7261732020203d202d392e343938383434363334333531343837652d303120332e313736313439343733353637383230652d303120322e303036363834323431343037303830652d30310a637261732020203d202d342e353336373032343338313339393238652b3031202d322e353238303839363238363934323838652b3031202d352e333030343339393139353839333937652b30310a76616c6964203d20300a 0 5 2 2 40400000 3fc00000 40000000 1 bf800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 42680000 42800000 c27865ae 00000000 00000000 00000000 x612062",
    ),
    (
        "FS-289",
        true,
        "smulf 4 1 3 10 2 0 219 3f000000 3f000000 3f800000 0 00000000 bf800000 00000000 00000000 00000000 3f800000 bf800000 00000000 00000000 41300000 cbae9355 4229be74 00000000 00000000 00000000 0 -2 42b69804 c0c00000 424efaf1 c2142181 c1100000 42b40000 54a386d8 41900000 42480000 429e0000 c29184e2 4053800000000000 3 428e0000 c2234208 0 1 null 3 x46726565537572666572436f6c6f724c55542e747874 -1 5 x736368656d61 1 x6c6162656c2d30 150 162 91 68 3f6ebc47 3d5a40db 3f4a5634 3e1f5d17 0 642 0 1 x 23 165 16 50 3efd820e 3f449a5d 3e3089a3 3e8063ec 3 964 null 1 3 3 10 2 0 -1 40000000 3f800000 3f700000 1 3e207899 bf1cad19 3ee2cd5c 3f4230e4 be23480a bf2db1ae 3e26797c beec89d3 bf378a42 c291865b c1200000 42a20000 00000000 00000000 00000000 -2 5 c0c00000 c158859c c29c3f87 c1bb106c c2542545 4218a711 4297964e 422f0908 4272921c 4291689e 7f800000 402bb430c70da390 5 42140000 c295a524 1 1 null null c2707996 0",
    ),
    (
        "FS-290",
        false,
        "com 1 1 1 10 3 1 53 3fc00000 40400000 3f000000 1 3f800000 00000000 00000000 00000000 bf800000 00000000 00000000 00000000 bf800000 4220d388 42700000 d67c0ed7 00000000 00000000 00000000 4 2 41087013 428be439 c1d3002e c1700000 609059a2 42b40000 c2451f00 42bc0000 42bc0000 00000000 42555cac 403d21f4179c3f60 8 423c0000 42400000 2 1 null null 0",
    ),
    (
        "FS-291",
        true,
        "invc 1 4 2 7 1 0 -1 3f700000 3f800000 3f000000 1 bf800000 00000000 00000000 00000000 00000000 bf800000 00000000 3f800000 00000000 c851a93d 41b80000 c29480ed 00000000 00000000 00000000 0 7 c29cd04b 41e9d502 42440000 c275a407 ffc00000 4253ca39 427e75a8 c28d1154 c2b82464 c0000000 c21c2248 43c2b1bc8f818c15 9 42bedaf2 41e00000 0 1 null null new c17bbb63",
    ),
    (
        "FS-293",
        true,
        "extreg 1 3 1 4 1 0 -1 3f700000 3fc00000 40000000 1 00000000 3f800000 00000000 bf800000 00000000 00000000 00000000 00000000 bf800000 417dc952 42580000 c27e9911 00000000 00000000 00000000 -2 8 421c0000 c2a7a793 428ebe79 419bbb9a c2c432b7 c0048496 4237c5cf c29032cc c218b8aa c296deae 48bb3d26 c057d179bfc49c56 0 c107ea25 42800000 1 1 null null 1 4 4 9 1 0 215 3f800000 40400000 40400000 0 3eede9b1 3f166299 3f41c629 3f278cee bec42217 bf3bc47a bd8cef55 bed5d9e8 3f360c6e 418ce727 c279a1be da3e9748 00000000 00000000 00000000 5 4 c1a2eb48 5ecda790 c1a00000 c0a00000 33305d67 c1300000 c0e00000 42540000 c1b09bfe 1fc8b689 42ac0000 4014000000000000 0 c29ffcb7 42adf7fb 0 0 null null null",
    ),
    (
        "FS-294",
        true,
        "extreg 1 4 5 0 2 0 8 40000000 40000000 3f800000 1 3f649d82 bf5b7169 3ea3a625 bf1174e5 bd1a034f 3f226087 bf0237e6 3e3b0277 bf700109 c2c2d685 4135e18b 41000000 00000000 00000000 00000000 5 0 42580000 c2136261 2819fae5 420119df 428db486 4211f4d4 c004f49b 42a105b8 c2006cd2 c29a9090 24a1d61f 4057000000000000 6 42040000 42a83c74 2 1 4 4 3fa1af19 bf1ab5e1 bc9b2d2c 421f07fa 3f9370da bfb8b814 3f9279d0 c1a5263e 3d971bd6 bfb18262 bf7824ab 41ce0cf1 00000000 00000000 00000000 3f800000 null new 2 -1 3 1 2 5",
    ),
    (
        "FS-295",
        true,
        "extreg 4 3 4 4 1 0 -1 40400000 40400000 3f700000 1 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 bf800000 00000000 42713d73 c2880a9e 40e00000 00000000 00000000 00000000 2 5 4272c3bc 400f7e2d c2322a92 42ac0000 42780000 426e4626 429e0000 40a00000 42640000 3dca1f06 c2784068 403d000000000000 4 c1bced90 c29f0eb1 1 1 null null new null",
    ),
    (
        "FS-332",
        true,
        "clone 1 1 4 10 3 0 -1 3f000000 3f700000 00000000 1 3f5ddabf 3ef1707d bdbbae24 bed64497 bf530dc0 3f48a348 be125a13 bd78f0a9 3d17552b 41400000 42c60000 c1ef02bd 00000000 00000000 00000000 0 5 41f9f6f7 40e00000 c277143a 4e3ad3ed 42bc3ddd c296e44d 42300000 c2837113 42680000 41900000 c1880000 bfd16eb82deefb00 2 42b20000 c16d0b63 2 1 null 2 x 2 0 x736368656d61 1 x556e6b6e6f776e 143 136 248 194 3e7266d3 3e502aaa 3ef8e071 3cc21c49 3 959 1 x6c6162656c2d31 74 53 115 196 3f528241 3f22317e 3f108a50 3ea1bc64 1 873 null self",
    ),
    (
        "FS-47",
        true,
        "pca 4 1 3 7 1 0 247 3fc00000 3fc00000 3fc00000 0 3f345413 bd5dd9ee 3f6c515f bf586c5d bf1c1710 bef36d7d 3ee80623 bf692107 bf7bd474 42c00000 c2abf5e0 42be0000 00000000 00000000 00000000 0 4 42b40000 42ae0000 42c1a6c0 42a6da41 42c60000 26256c2c c1ca6139 c2458487 c28a4e16 42640000 42014f23 c0187b579db75640 3 40cd4ad4 4242efb8 2 1 null null 10",
    ),
    (
        "ITK-112",
        true,
        "pca 4 4 1 3 2 0 t14 3f800000 3f800000 3fc00000 1 00000000 00000000 bf800000 00000000 3f800000 00000000 bf800000 00000000 00000000 42950697 41300000 42080000 00000000 00000000 00000000 2 5 c2b89274 b7260669 c27812da 4275bb07 40ed02e2 40c00000 42860000 41900000 c0e00000 41a00000 4243bb90 4053932f07bbc54c 7 d282b79e 426f3681 0 1 null null 0",
    ),
    (
        "FS-52",
        true,
        "abs 1 3 4 0 1 0 42 40000000 3f000000 3f000000 1 bf800000 00000000 00000000 00000000 bf800000 00000000 00000000 00000000 3f800000 41d80000 c0400000 c0a00000 00000000 00000000 00000000 5 8 42ba3116 c2b91fdd 42a2439a 4264e20f 42c03940 422b511b c0800000 585792cc 52d97c44 c237a342 40400000 c05177c7ecf3764f 1 42880000 80000000 1 1 null null 2 3 2 2 2 0 -1 3f800000 40400000 3fc00000 0 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 bf800000 00000001 c1b3cf8c 428c0000 00000000 00000000 00000000 4 3 a7c23de1 41fa1b00 bd5c511a 7f800000 b9ca4b4a 42c60000 42500000 42a86b32 42b60000 4251e0c1 42140000 4048000000000000 4 42ba0000 c26ec176 2 1 null null",
    ),
    (
        "FS-53",
        false,
        "cpframe 1 2 1 9 2 0 -1 40000000 40000000 3f700000 1 00000000 00000000 bf800000 3f800000 00000000 00000000 00000000 bf800000 00000000 c0400000 c1880000 42b60000 00000000 00000000 00000000 2 0 426ae677 41b80000 c266e46f 2153595a 41de0422 c1600000 40eee4d2 41b00000 42b60000 c1a00000 41fc1032 c032eccd08cc306c 3 41e80000 42992c05 2 1 null null self 2 0",
    ),
    (
        "FS-58",
        true,
        "clear 3 2 1 2 3 1 -1 40400000 3fc00000 3f800000 1 bf61bc18 3ea44670 bf44052b 3df25a5c 3ef52505 bf1020ab be91dc8f bf34d67f 3f4b6c24 268163d0 42080000 c29d3d56 00000000 00000000 00000000 1 6 41e80000 42b00000 3f4fa2d1 c1957c1c c2b7dd64 42b6770f 4190e32b c28b20d5 c24d0701 42ad73c3 41d20376 c0522641ab56ab8c 8 3782c9cc 4294f40e 2 1 null null",
    ),
    (
        "FS-12",
        true,
        "smul 1 2 4 9 1 0 79 3f000000 3f700000 3f800000 0 3f068259 bf5e0192 bf1be4f0 3f7ffb3f 3ec90923 be0d8bca 3f33efbb be2912d2 bf402963 7f7fffff 7fc00000 42428187 00000000 00000000 00000000 4 6 310a9efc 4199341a c0000000 42b13ecd 282d8b42 c2a28eeb 40ca3f32 a7b2bc0d 42600000 41800000 c1880000 40423974425ebc50 9 c2832741 c1880000 0 1 null null self c0e00000",
    ),
    (
        "FS-265",
        false,
        "vrangereg 2 2 2 3 1 0 5 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null null",
    ),
    (
        "FS-298",
        true,
        "roty 3 4 4 2 2 0 66 3f700000 00000000 3f700000 1 3f800000 00000000 00000000 00000000 bf800000 00000000 00000000 00000000 3f800000 c19cf626 42600000 42400000 00000000 00000000 00000000 2 0 42b86e5e 42c60000 b918e4ea 42a5f662 ffc00000 421c0000 429de331 c28ceccf 425b60ce c2c20207 c274e26a be32cf03992708c3 1 428a0000 41900000 0 1 null null new bee2cd17",
    ),
    (
        "FS-299",
        true,
        "samp cub 3 3 3 10 1 0 92 3f800000 3fc00000 3fc00000 1 3f5a7908 bf295a3b bf42f347 3f38fe13 3f5635a2 bf156b1a bf7538dd 3d7d1a62 3f0a109c 42015d09 a4a6ed87 42a00000 00000000 00000000 00000000 2 2 4291ac99 42700000 4ff7ddad 428ccd3f 4217e460 425b129f 42800000 41980000 40e40b8a 4136f6c4 42000000 c054da9602f01281 1 426c894c 7f7fffff 1 1 null null 7e37e43c8800759c 1 3e8ad7f29abcaf48 3ff000001ff19e24 3e7ff19e23a836fc 2 0",
    ),
    (
        "FS-300",
        false,
        "samp cubf 1 4 4 2 3 0 -1 3f800000 3f800000 3f800000 1 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 00000000 00000000 c20d381d 42b00000 42340000 00000000 00000000 00000000 3 5 c29be39e bffdef3c ba266780 40800000 c2854fcd 42540000 c2154b02 c6bb656f 4b393ea8 c2c3a834 422b7825 40e9fe3850d07eaa 2 42b80000 42600000 2 1 null null 0000000000000000 1 3fd5539c7e463df0 0000000000000000 be45798ee2308c3a 0 0",
    ),
    (
        "FS-301",
        true,
        "rotx 2 2 3 6 2 0 7 3f700000 3f800000 3fc00000 1 00000000 00000000 3f800000 00000000 bf800000 00000000 bf800000 00000000 00000000 42c3bb2b 42280000 c101d35e 00000000 00000000 00000000 2 8 42c60000 7f800000 42380000 40400000 41900000 a245d637 c2302cb8 42780000 4250a203 420c0000 41c80000 c017274365c7e510 0 bf800000 bfa84141 0 2 null null new 00000000",
    ),
    (
        "FS-303",
        true,
        "rotx 2 2 3 6 2 0 7 3f700000 3f800000 3fc00000 1 00000000 00000000 3f800000 00000000 bf800000 00000000 bf800000 00000000 00000000 42c3bb2b 42280000 c101d35e 00000000 00000000 00000000 2 8 42c60000 7f800000 42380000 40400000 41900000 a245d637 c2302cb8 42780000 4250a203 420c0000 41c80000 c017274365c7e510 0 bf800000 bfa84141 0 2 null null new 00000000",
    ),
    (
        "FS-304",
        true,
        "rotz 4 4 3 10 2 0 2 40400000 3f800000 3f700000 0 be81da55 3df8718f 3f316868 bf0dc223 3e858403 3f48402b becf733b bebf69e7 3e8bc07e 42843b77 42a40000 429aa945 00000000 00000000 00000000 4 1 42000000 42afdb88 41852a63 bf9f1dc1 c2c3071c 428a0000 c2ba8e54 c2a0a768 42ac0000 41e578c5 b85e4878 40539b46ace17328 4 42400000 c1fd4d0c 2 0 null null new 7fc00000",
    ),
    (
        "FS-306",
        false,
        "tftype 1 x616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161612e6c7461",
    ),
    (
        "FS-307",
        true,
        "ltamem x662e78666d x4d4e49205472616e73666f726d2046696c650a25610a25620a25630a 0 none",
    ),
    (
        "FS-308",
        false,
        "ltamem x74 x747970203d20310a6e78666f726d73203d20300a7375626a65637420626572740a 0 none",
    ),
    ("FS-309", true, "ltamem x74 x74 2 none"),
    (
        "FS-310",
        true,
        "ltamem x672e78666d x4d4e49205472616e73666f726d2046696c650a2547656e6572617465642062792078206473740a0a5472616e73666f726d5f54797065203d204c696e6561723b0a4c696e6561725f5472616e73666f726d203d0a312030203020300a302031203020300a302030203120303b0a 0 none",
    ),
    (
        "FS-320",
        true,
        "splat 2 4 4 0 2 0 -1 40000000 00000001 40000000 0 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 7fc12345 00000000 ce3f3aa3 c240839a 4230e995 00000000 00000000 00000000 5 6 42b6e0e4 40c00000 42540000 4ca36c99 4180a3c0 4295bd2f c225795c bccf5c9a 41100000 c0c00000 428194db 404d800000000000 8 42c80000 c271f285 2 2 null null 2 1 3fe316949f4e0cb8 0000000000000000 3feffffffaa19c47 0 c024000000000000 0 4202a05f20000000 be45798ee2308c3a bff0000000000000 0 406fe00000000000",
    ),
    (
        "FS-321",
        true,
        "splat 4 3 1 2 1 0 74 3f000000 40400000 3fc00000 1 00000000 bf800000 00000000 3f800000 00000000 00000000 00000000 00000000 bf800000 c2c3d6a4 42700000 3982c335 00000000 00000000 00000000 1 0 4cba0681 00000001 42b47726 409b870b 42622542 4206f4e4 42000000 40400000 40883b46 c2856032 b7362f91 4054c00000000000 3 42b95ec0 41f00000 1 0 null null 1 1 3fce6ea49cb9c9b0 bfc3693443fbf388 3fafa9c7ab62dc60 0 4004000000000000",
    ),
    (
        "FS-322",
        true,
        "deriv 3 5 4 9 1 0 211 40400000 3f000000 3fc00000 0 00000000 bf800000 00000000 3f800000 00000000 00000000 00000000 00000000 bf800000 42b45ed8 ffc00000 40a542e1 00000000 00000000 00000000 3 1 42480000 c1a00000 518186b6 42980000 c2c04fc0 c032d73a c245e1d6 c2c029ae 400d7042 41500000 427c0000 c030000000000000 2 419f36be 42a60000 1 2 null null 7ff8000000000000 1 1 3fff40a82c5ed548 40077d55a1fa02d2 4000000000000000 bfc2e517323b3ab8 bff0000000000000 0000000000000000 0000000000000000",
    ),
    (
        "FS-325",
        true,
        "lxf 0 3 3 3 10 2 0 -1 3f800000 40000000 40000000 0 3de1f89f be9103b6 3e21e793 3edeff30 bf1029b1 3f70ca4b 3f3fe63f 3eda74c5 bf24f5c4 42a23295 c214f994 42820000 00000000 00000000 00000000 1 8 42c20000 417da752 41c00000 40800000 426cb493 4292a43e c26c8f84 41c8df6b 40800000 7fc00000 42b36744 c03e8dcd0dcb3f18 1 c2a43497 426c0000 2 1 null null 3e112e0be826d695 new 4 4 be96f6d0 3ef69821 bf2b5fef 42f5f497 bf258514 3ea87592 3eebdc2a 42d2242c 3da3f54b 3f177b24 3d388bf7 41fa4c7f 00000000 00000000 00000000 3f800000 2",
    ),
    (
        "FS-327",
        true,
        "rsfill 1 3 3 3 2 0 194 3fc00000 3f000000 3f700000 1 00000000 bf800000 00000000 00000000 00000000 bf800000 3f800000 00000000 00000000 b877e51c d32920fc 4ad595a2 3e4ccccd bdcccccd 00000000 0 4 42300000 41880000 42b00000 425c0000 42c60000 dc9ea049 42740000 420ca03e c2863911 42820000 42080000 4030000000000000 7 c1800000 424c0000 1 1 null null 2 1 4 10 2 0 -1 3f800000 40000000 40400000 0 7fc00000 00000000 bf800000 00000000 bf800000 00000000 bf800000 00000000 00000000 42a60000 c2371b8f 42040000 00000000 00000000 00000000 -1 2 c24a8b58 c223980a c28eb612 42840000 41546106 c1ab95f4 4270c6f2 c18a3ae3 352b91cf 41200000 421f026f c03366129f41e868 2 c1000000 41b8b77c 2 1 null null 6 477fff00 0",
    ),
    (
        "FS-328",
        true,
        "rsfill 4 2 3 2 1 0 -1 3fc00000 40000000 3f700000 1 bf7c5c92 bf5dd61d bf452a79 3f04658d bec1b426 3f3cf989 be0162a7 3d3cf185 bedd9868 42375964 c28dc579 42bf1dcf 00000000 00000000 00000000 4 7 42ad85ba 42a54271 42c604df 4294ed25 42480000 c0a770f7 c28d1b88 42131c60 41300000 c2b6da84 422f0624 401550004c8453d0 8 41f80000 c1980000 0 1 null null 4 2 2 3 1 0 225 40000000 3f000000 3f000000 1 00000000 00000000 bf800000 3f800000 00000000 00000000 00000000 bf800000 00000000 c2b40823 41e80000 a7c5641e 3e4ccccd bdcccccd 3e4ccccd 4 3 c76c0610 41b00000 c2c231d0 423535f3 c1db4519 c284adff 41880000 80000000 3a577d20 a609b8ed 429c0000 4050400000000000 6 c0e00000 c281f057 2 1 null null 4 437f8000 0",
    ),
];

/// The line in `mode`: its result and streams without the notices, and the IDs noted.
fn run(line: &str, mode: UpstreamDefects) -> (String, Vec<String>) {
    let toks: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    let r = volume_cmd::run_line(&toks, mode, &Environment::new());
    let (res, streams) = r.rsplit_once(" | ").expect("a result line");
    let (out, err) = streams.split_once(' ').expect("two streams");
    let err: Vec<u8> = (1..err.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&err[i..i + 2], 16).expect("hex"))
        .collect();
    let err = String::from_utf8_lossy(&err).into_owned();
    let mut ids = Vec::new();
    let mut kept = String::new();
    for l in err.split_inclusive('\n') {
        match l.strip_prefix("upstream defect ") {
            Some(rest) => ids.push(rest.split(' ').next().unwrap_or("").to_owned()),
            None => kept.push_str(l),
        }
    }
    (format!("{res} | {out} {kept}"), ids)
}

fn check(id: &str, differs: bool, line: &str) {
    let (fix, fix_ids) = run(line, UpstreamDefects::Fix);
    let (rep, rep_ids) = run(line, UpstreamDefects::Reproduce);
    assert!(
        fix_ids.iter().any(|i| i == id),
        "{id} not noted in fix: {line}"
    );
    assert!(
        rep_ids.iter().any(|i| i == id),
        "{id} not noted in reproduce: {line}"
    );
    assert_eq!(fix != rep, differs, "{id}: the modes' results {line}");
}

#[test]
fn each_defect_is_noted_in_both_modes() {
    let failed: Vec<&str> = CASES
        .iter()
        .filter(|&&(id, differs, line)| {
            std::panic::catch_unwind(|| check(id, differs, line)).is_err()
        })
        .map(|&(id, _, _)| id)
        .collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}

/// A working volume (README.md's W) of shape `v` and fill `fill`: unit voxels, the coronal
/// cosines, the header fields 0, the cache recomputed, no registration matrix, no colour table.
fn w(v: &str, fill: &str) -> String {
    format!(
        "{v} {fill} 3f800000 3f800000 3f800000 1 bf800000 00000000 00000000 00000000 00000000 \
         bf800000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 \
         0 0 {} 0000000000000000 1 00000000 00000000 0 1 null null",
        ["00000000"; 11].join(" ")
    )
}

/// FS-292: three byte volumes (the first one also the destination), a zero divisor in the
/// middle of the row: FreeSurfer stays at the same place in the first volume and the destination.
#[test]
fn fs292_division_after_a_zero_divisor() {
    let line = format!(
        "div2 {} {} self",
        w("3 1 1 0 1 0", "200"),
        w("3 1 1 0 1 0", "155")
    );
    check("FS-292", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    // the voxels: 200 / 155, 45 / 0, 146 / 101 (default 1 255 1; FreeSurfer 1 2 146)
    assert!(fix.contains("x01ff01 "), "{fix}");
    assert!(rep.contains("x010292 "), "{rep}");
}

/// FS-296: a NaN product stored into a byte voxel is 0 in both modes.
#[test]
fn fs296_nan_into_a_byte() {
    check(
        "FS-296",
        false,
        &format!("smulf {} new 7fc00000 0", w("2 1 1 0 1 0", "7")),
    );
}

/// FS-297: a box of 64-bit voxels from column 1: FreeSurfer copies from half the first column.
#[test]
fn fs297_long_box() {
    check(
        "FS-297",
        true,
        &format!("extinto {} new 1 0 0 1 1 1 0 0 0", w("3 1 1 2 1 0", "9")),
    );
}

/// FS-302: a sinc window of 138 voxels on an axis ends the program in both modes (FreeSurfer
/// writes past its 128 weights).
#[test]
fn fs302_a_sinc_window_beyond_128_voxels() {
    let x = 70.5f64.to_bits();
    let line = format!(
        "samp sinc {} 0000000000000000 1 {x:016x} 0000000000000000 0000000000000000 0 70",
        w("140 1 1 0 1 0", "5")
    );
    check("FS-302", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    assert!(fix.starts_with("EXIT 252 |"), "{fix}");
}

/// FS-305: an infinite voxel makes the centre of mass not a number: the program ends in both
/// modes, before any walk.
#[test]
fn fs305_a_centre_of_mass_that_is_not_a_number() {
    let line = format!(
        "skull {} 1 2 0 0 0 0 0 0 7f800000 3 3 3 8 8 8 43480000",
        w("12 12 12 3 1 0", "-1")
    );
    check("FS-305", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    assert!(fix.starts_with("EXIT 252"), "{fix}");
}

/// FS-258: a register.dat's subject of 1,000 bytes is kept whole, noted in both modes.
#[test]
fn fs258_a_register_dat_subject_of_1000_bytes() {
    let mut text = "a".repeat(1000);
    text.push_str("\n1\n1\n0.15\n");
    for r in 0..4 {
        for c in 0..4 {
            text.push_str(if r == c { "1 " } else { "0 " });
        }
        text.push('\n');
    }
    text.push_str("round\n");
    let hex = |b: &[u8]| -> String {
        let mut s = "x".to_owned();
        for c in b {
            s.push_str(&format!("{c:02x}"));
        }
        s
    };
    let line = format!("ltamem {} {} 0 none", hex(b"t.dat"), hex(text.as_bytes()));
    check("FS-258", false, &line);
}

/// A float working volume of shape `v` (frame 0 fill `fill`), painted (README.md's P).
fn wp(v: &str, fill: &str, paint: &str) -> String {
    format!("{} {paint}", w(v, fill))
}

/// FS-311: a NaN voxel as a label: -2147483648 with `reproduce` (unrecognized, kept), 0 by
/// default (its own contralateral label).
#[test]
fn fs311_a_nan_label() {
    let line = format!("lrswap {}", wp("1 1 1 3 1 0", "0", "1 0 0 0 7fc00000"));
    check("FS-311", true, &line);
}

/// FS-312: a thalamus voxel outside the ribbon counts twice with `reproduce`.
#[test]
fn fs312_the_thalamus_twice() {
    let line = format!(
        "cmavol {} {} 0",
        wp("1 1 1 3 1 0", "0", "1 0 0 0 41200000"),
        wp("1 1 1 3 1 0", "0", "0")
    );
    check("FS-312", true, &line);
}

/// FS-313: the right vessel (62) in the ribbon counts by default, not with `reproduce`.
#[test]
fn fs313_the_right_vessel() {
    let line = format!(
        "cmavol {} {} 0",
        wp("1 1 1 3 1 0", "0", "1 0 0 0 42780000"),
        wp("1 1 1 3 1 0", "0", "0")
    );
    check("FS-313", true, &line);
}

/// FS-314: a cache line naming a measure without a number and ending in white space: by default it
/// gives no number (the measure then missing: status 1); `reproduce` aborts with the C++ runtime's
/// two lines (status 134), `std::bad_alloc` for words of at most 15 bytes, `std::length_error`
/// past that.
#[test]
fn fs314_a_cache_line_without_a_number() {
    let bv = |text: &str| {
        let mut hex = "x".to_owned();
        for c in text.bytes() {
            hex.push_str(&format!("{c:02x}"));
        }
        format!("bvparse {hex}")
    };
    let line = bv("# Measure BrainSeg, BrainSegVol, Brain Segmentation Volume, x, mm^3 \n");
    check("FS-314", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    assert!(fix.starts_with("EXIT 1 |"), "{fix}");
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    assert!(rep.starts_with("EXIT 134 |"), "{rep}");
    assert!(
        rep.contains("'std::bad_alloc'\n  what():  std::bad_alloc\n"),
        "{rep}"
    );
    let line = bv("# Measure SupraTentorialNotVentVox, SupraTentorialVolNotVentVox, x, mm^3\t\n");
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    assert!(
        rep.contains("'std::length_error'\n  what():  basic_string::_M_create\n"),
        "{rep}"
    );
}

/// FS-315: label lists of 2 x 0: the default ends the program (code -4, before any voxel is
/// written); `reproduce` reads their numbers as NaN (no swap).
#[test]
fn fs315_label_lists_without_numbers() {
    let line = format!(
        "lrseg {} 2 0 2 0",
        wp("2 1 1 3 1 0", "0", "1 1 0 0 40000000")
    );
    check("FS-315", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    assert!(fix.starts_with("EXIT 252 |"), "{fix}");
    assert!(
        fix.contains("MRIlrswapSeg(): the label lists hold fewer numbers than rows (FreeSurfer reads past them)\n"),
        "{fix}"
    );
    assert!(
        !rep.starts_with("null |") && !rep.starts_with("EXIT"),
        "{rep}"
    );
}

/// FS-960: a tensor volume's rows walked directly (FS-291's walk of `MRIinvertContrast`,
/// `reproduce` only: the default refuses that walk, FS-291, so no function of parts 1 to 5 walks
/// a tensor by default): every row at the first voxel, noted.
#[test]
fn fs960_a_tensor_volumes_rows() {
    let line = format!("invc {} new 3f800000", w("1 2 1 6 1 0", "9"));
    let (_, rep_ids) = run(&line, UpstreamDefects::Reproduce);
    assert!(rep_ids.iter().any(|i| i == "FS-960"), "{rep_ids:?}");
    let (_, fix_ids) = run(&line, UpstreamDefects::Fix);
    assert!(fix_ids.iter().any(|i| i == "FS-291"), "{fix_ids:?}");
}

/// FS-316: a flood fill of a 4 x 2 byte volume seeded at a row past the height (row 5, threshold
/// 0): by default at the last row (the whole volume filled, npix 8); with `reproduce` at row 3
/// (the width minus one), outside the volume (nothing filled, npix 1).
#[test]
fn fs316_a_seed_row_past_the_height() {
    let line = format!("fillfg {} new 0 5 0 0 1", w("4 2 1 0 1 0", "200"));
    check("FS-316", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    assert!(fix.starts_with("8 "), "{fix}");
    assert!(rep.starts_with("1 "), "{rep}");
}

/// FS-317: a downsampling factor of 0 (§26: not in the probe's cases): with `reproduce` the
/// extent's infinite quotient becomes -2147483648 and the allocation refuses it (the default's
/// 2147483647 would ask for a volume of that width, so only `reproduce` runs here; the
/// conversion's default is the crate's unit test).
#[test]
fn fs317_a_factor_of_zero() {
    let line = format!(
        "dnsn {} new 00000000 3f800000 3f800000 0",
        w("1 1 1 3 1 0", "t3")
    );
    let (rep, ids) = run(&line, UpstreamDefects::Reproduce);
    assert!(ids.iter().any(|i| i == "FS-317"), "{ids:?}");
    assert!(rep.starts_with("EXIT 1 |"), "{rep}");
}

/// FS-318: the mean of frame 2 of a three-frame volume: frame 0's with `reproduce`, frame 2's by
/// default.
#[test]
fn fs318_the_mean_of_another_frame() {
    let line = "mft 1 2 2 9 3 0 -1 3f700000 3f800000 3f700000 1 be46941b 3f2efe46 befaaa3a 3f11b6f8 \
                3ee94a94 bdd0293b 3eaa2836 bc84ee3e bf33501b 4280cbc5 420f6f25 42980000 00000000 \
                00000000 00000000 0 2 c24a93e7 41c80000 c0800000 c20c24e3 40e00000 a14e1b78 42380000 \
                40c00000 ff800000 c160f703 40cab906 4055000000000000 1 c2c5623b c0e00000 1 2 null \
                null 2 41e1e4da";
    check("FS-318", true, line);
}

/// FS-323: a neighbour count of a voxel 600 columns away (past the boundary index tables, -500
/// to the extent + 499) ends the program in both modes, before any voxel is read.
#[test]
fn fs323_an_index_past_the_tables() {
    let line = format!("nbrs {} 1 0 600 0 0 1 0 00000000 00000000", w("2 1 1 0 1 0", "5"));
    check("FS-323", true, &line);
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, _) = run(&line, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(
            r.contains("MRIneighborsOn: the voxel index 599 lies past the volume's index tables (FreeSurfer reads memory it does not own)\n"),
            "{r}"
        );
    }
}

/// FS-324: the derivative at an infinite scale, and at 1e308 (twice which overflows), ends the
/// program in both modes, the scale as Rust writes it.
#[test]
fn fs324_a_scale_too_large() {
    for (bits, text) in [
        ("7ff0000000000000", "inf".to_owned()),
        ("7fe1ccf385ebc8a0", format!("{}", 1e308f64)),
    ] {
        let line = format!(
            "deriv {} 3ff0000000000000 1 1 3ff0000000000000 0 0 3ff0000000000000 0 0 {bits}",
            w("3 1 1 3 1 0", "t2")
        );
        check("FS-324", true, &line);
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (r, _) = run(&line, mode);
            assert!(r.starts_with("EXIT 252 |"), "{r}");
            let want = format!(
                "MRIsampleVolumeDerivativeScale: the scale {text} is too large (FreeSurfer never returns)\n"
            );
            assert!(r.contains(&want), "{r}");
        }
    }
}

/// A one-voxel byte volume of `frames` frames, voxel f of frame f `value(f)`.
fn bytes(p: &mut Process, frames: i32, value: impl Fn(i32) -> f32) -> Mri {
    let mut m = p
        .mri_alloc_sequence(1, 1, 1, types::UCHAR, frames)
        .expect("a volume");
    for f in 0..frames {
        p.mri_set_vox_val(&mut m, 0, 0, 0, f, value(f), 0)
            .expect("a voxel");
    }
    m
}

/// Whether the process noted `id`.
fn noted(p: &mut Process, id: &str) -> bool {
    p.take_notices()
        .as_slice()
        .iter()
        .any(|n| n.defect.id == id)
}

/// FS-326: 280 votes for label 1 and 320 for label 2 in one voxel (a source of 600 frames on a
/// template of its own grid): by default label 2; with `reproduce` both counts stop at 255 and the
/// lower label, 1, wins.
#[test]
fn fs326_votes_past_255() {
    for (mode, want) in [(UpstreamDefects::Fix, 2.0), (UpstreamDefects::Reproduce, 1.0)] {
        let mut p = Process::silent(Environment::new(), mode);
        let src = bytes(&mut p, 600, |f| if f < 280 { 1.0 } else { 2.0 });
        let template = bytes(&mut p, 1, |_| 0.0);
        let r = p
            .mri_resample_fill(&src, &template, 6, 0.0)
            .expect("no end")
            .expect("a volume");
        assert_eq!(p.mri_get_vox_val(&r, 0, 0, 0, 0, 0).expect("a voxel"), want);
        assert!(noted(&mut p, "FS-326"));
    }
}

/// FS-327: voting onto a template whose orientation is unknown (no good RAS): the error and no
/// volume in both modes (FreeSurfer crashes).
#[test]
fn fs327_a_vote_without_a_matrix() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut p = Process::silent(Environment::new(), mode);
        let src = bytes(&mut p, 1, |_| 3.0);
        let mut template = bytes(&mut p, 1, |_| 0.0);
        template.geom.ras_good_flag = 0;
        let r = p
            .mri_resample_fill(&src, &template, 6, 0.0)
            .expect("no end");
        assert!(r.is_none());
        assert!(noted(&mut p, "FS-327"));
    }
}

/// FS-461: a value replaced in a new destination of a two-frame byte volume: by default in both
/// frames; with `reproduce` in frame 0 only (frame 1 of the new destination stays 0).
#[test]
fn fs461_both_frames_of_a_byte_volume() {
    for (mode, want) in [(UpstreamDefects::Fix, 9.0), (UpstreamDefects::Reproduce, 0.0)] {
        let mut p = Process::silent(Environment::new(), mode);
        let mut src = bytes(&mut p, 2, |_| 5.0);
        let r = p
            .mri_replace_values_uchar(&mut src, Dst::New, 5, 9)
            .expect("no end");
        let larmorx_fs_core::volume::VolOut::New(d) = r else {
            panic!("a new volume");
        };
        assert_eq!(p.mri_get_vox_val(&d, 0, 0, 0, 0, 0).expect("a voxel"), 9.0);
        assert_eq!(p.mri_get_vox_val(&d, 0, 0, 0, 1, 0).expect("a voxel"), want);
        assert!(noted(&mut p, "FS-461"));
    }
}

