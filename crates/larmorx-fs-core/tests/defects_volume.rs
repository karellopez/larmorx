// SPDX-License-Identifier: Apache-2.0
//! fs.volume's defects as D13 switches (`specs/freesurfer-volume.md` §69.4, parts 1 to 8; fs.bspline's,
//! `specs/freesurfer-bspline.md` §7): each one noted in
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
    let line = format!(
        "nbrs {} 1 0 600 0 0 1 0 00000000 00000000",
        w("2 1 1 0 1 0", "5")
    );
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
    for (mode, want) in [
        (UpstreamDefects::Fix, 2.0),
        (UpstreamDefects::Reproduce, 1.0),
    ] {
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
    for (mode, want) in [
        (UpstreamDefects::Fix, 9.0),
        (UpstreamDefects::Reproduce, 0.0),
    ] {
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

// ---- part 7a (§33)

/// FS-329: the statistics of a 64-bit volume of two voxels 5 and 7: their own values by default
/// (mean 6), 32-bit integers at the voxels' indices with `reproduce` (voxel 5's two halves: 5
/// and 0, mean 2.5).
#[test]
fn fs329_a_64_bit_volume() {
    for (mode, want) in [
        (UpstreamDefects::Fix, (5.0f32, 7.0f32, 6.0f32)),
        (UpstreamDefects::Reproduce, (0.0, 5.0, 2.5)),
    ] {
        let mut p = Process::silent(Environment::new(), mode);
        let mut m = p
            .mri_alloc_sequence(2, 1, 1, types::LONG, 1)
            .expect("a volume");
        let v = m.voxels.as_mut().expect("voxels");
        v[..8].copy_from_slice(&5i64.to_le_bytes());
        v[8..16].copy_from_slice(&7i64.to_le_bytes());
        let s = p.mri_stats(&m).expect("statistics");
        assert_eq!((s.min, s.max, s.n), (Some(want.0), Some(want.1), 2));
        assert_eq!(s.mean, want.2);
        assert!(noted(&mut p, "FS-329"));
    }
}

/// FS-330: the printed statistics of an RGB volume: its minimum and maximum, never written, 0
/// in both modes, noted.
#[test]
fn fs330_a_type_the_statistics_do_not_read() {
    let line = format!("pstats {}", w("2 1 1 9 1 0", "7"));
    // the modes differ only by FS-01's NUL in the centre of mass's errors
    check("FS-330", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    let want: String = b"2 values\nmin = 0\nmax = 0\n"
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert!(fix.contains(&format!("| x{want}")), "{fix}");
}

/// FS-462: the centre of a volume sampled onto a destination of no columns ends the program in
/// both modes (FreeSurfer divides by the extent and crashes).
#[test]
fn fs462_a_sampled_extent_of_zero() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut p = Process::silent(Environment::new(), mode);
        let mut src = bytes(&mut p, 1, |_| 3.0);
        let mut dst = bytes(&mut p, 1, |_| 0.0);
        dst.geom.width = 0;
        let r = p.mri_calc_cras_for_sampled_volume(&mut src, &dst);
        assert!(matches!(r, Err(s) if s.status() == 252), "{r:?}");
        assert!(noted(&mut p, "FS-462"));
    }
    // the probe's command (a destination of width 0: its allocation's message on standard output)
    let line = format!("crass {} {}", w("2 2 2 0 1 0", "5"), w("0 2 2 0 1 0", "5"));
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    assert!(fix.starts_with("EXIT 252 | x"), "{fix}");
    assert!(
        fix.contains(
            "error: MRIcalcCRASforSampledVolume(): an extent of the sampled volume is 0 (FreeSurfer crashes here)\n\n"
        ),
        "{fix}"
    );
}

/// FS-463: an orientation string of one character: the places past its end read as NUL in both
/// modes (FreeSurfer reads memory contents), noted.
#[test]
fn fs463_a_string_of_one_character() {
    check("FS-463", false, "chkostr x4c");
    let (fix, _) = run("chkostr x4c", UpstreamDefects::Fix);
    let want: String =
        b"Character Character   PA axis not represented.\n  IS axis not represented.\n"
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
    assert!(fix.starts_with(&format!("x{want} |")), "{fix}");
}

/// FS-464: a voxel-to-RAS matrix of 2 x 2, of 3 x 3 and none: the program's end in both modes
/// (`ERROR_BADPARM`, status 252, before anything is read or printed; FreeSurfer reads past the
/// matrix, or crashes without one).
#[test]
fn fs464_a_matrix_smaller_than_3_by_4() {
    let v = w("2 2 2 0 1 0", "5");
    let nine = ["3f800000"; 9].join(" ");
    for m in [
        "2 2 3f800000 00000000 00000000 3f800000".to_owned(),
        format!("3 3 {nine}"),
        "null".to_owned(),
    ] {
        let line = format!("setv2r {v} {m} unset");
        // the modes differ only by FS-01's NUL in the error
        check("FS-464", true, &line);
        let (fix, _) = run(&line, UpstreamDefects::Fix);
        assert!(fix.starts_with("EXIT 252 | x "), "{fix}");
        assert!(
            fix.ends_with(
                " error: MRIsetVoxelToRasXform(): the matrix is smaller than 3 x 4 (FreeSurfer reads past it)\n\n"
            ),
            "{fix}"
        );
    }
}

// ---- fs.bspline (specs/freesurfer-bspline.md §7)

/// FS-962: doubling a 4 x 1 x 1 volume: by default the expanded row stays; with `reproduce` pass y
/// copies the source's voxels at the destination's columns (columns 1 to 7 of a source of 4: past
/// it from column 4), noted in both modes.
#[test]
fn fs962_an_axis_of_one_voxel_doubled() {
    let line = format!("bsup {} new", w("4 1 1 3 1 0", "t6"));
    check("FS-962", true, &line);
}

/// FS-963: halving a 4 x 4 x 1 volume: the depth's voxel size doubles in both modes (FreeSurfer's
/// geometry), noted.
#[test]
fn fs963_an_axis_of_one_voxel_halved() {
    let line = format!("bsdown {} new", w("4 4 1 3 1 0", "t6"));
    check("FS-963", false, &line);
}

/// FS-964: sampling coefficients of degree 10 or -1 on one voxel: FS-964's end in both modes (its
/// error, status 252, nothing on standard output); degree 10 on 3 x 3 x 3: FS-964's end by
/// default, FreeSurfer's (`Invalid spline degree` on standard output, status 1) with `reproduce`.
#[test]
fn fs964_degrees_past_the_arrays() {
    let mid = "3ff8000000000000 3fd0000000000000 0000000000000000";
    let at0 = "0000000000000000 0000000000000000 0000000000000000";
    let end = |deg: &str| {
        format!(
            " error: MRIsampleBSpline: a B-spline of degree {deg} (FreeSurfer reads or writes memory it does not own here)\n\n"
        )
    };
    for deg in ["10", "-1"] {
        let line = format!(
            "bsraw {deg} 0 {} 0000000000000000 1 {at0} 0",
            w("1 1 1 3 1 0", "t22")
        );
        // the modes differ only by FS-01's NUL in the error
        check("FS-964", true, &line);
        let (fix, _) = run(&line, UpstreamDefects::Fix);
        assert!(fix.starts_with("EXIT 252 | x "), "{fix}");
        assert!(fix.ends_with(&end(deg)), "{fix}");
    }
    let line = format!(
        "bsraw 10 0 {} 0000000000000000 1 {mid} 0",
        w("3 3 3 3 1 0", "t22")
    );
    check("FS-964", true, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    assert!(fix.starts_with("EXIT 252 | x "), "{fix}");
    assert!(fix.ends_with(&end("10")), "{fix}");
    assert!(rep.starts_with("EXIT 1 |"), "{rep}");
}

/// A physical-voxel transform array changed to RAS-to-RAS (§54; a line of the probe's `vltapv`):
/// FS-506 (by default the voxel-to-voxel matrix made RAS-to-RAS, with `reproduce` FreeSurfer's
/// products) and FS-505 (its inverse without a matrix: kept by default, none with `reproduce`),
/// both noted in both modes.
const LTACHG_PHYSVOX: &str = "ltachg 1 2 3f800000 x 00000000 00000000 00000000 00000000 4 4 bfa24831 3fa0b602 3e07183b c2cfd969 bdad9c1b 3e7b6ae0 3eb5071a c026f0ff bf8a65a9 3eb7f858 3f4eed0c c21c60e8 00000000 00000000 00000000 3f800000 null null 0 0 1 5 1 3 3f800000 3fc00000 3f000000 1 3e67b509 3f3cb479 bf04ee6a be5dc2c7 3e0e4064 3f107d9d bf2f67ee bec1f57c bd109066 41c80000 42c60000 42240000 358637bd 3e4ccccd 3c23d70a x612062 0 null 1 3 3 2 3f800000 40400000 40000000 0 00000000 3f800000 00000000 bf800000 00000000 00000000 00000000 00000000 3f800000 426e0746 42100000 4173d041 00000000 00000000 00000000 x 0 null 00000000 00000000 00000000 00000000 null null null 0 0 1 3 3 2 3f800000 40400000 40000000 0 00000000 3f800000 00000000 bf800000 00000000 00000000 00000000 00000000 3f800000 426e0746 42100000 4173d041 00000000 00000000 00000000 x 0 null 1 5 1 3 3f800000 3fc00000 3f000000 1 3e67b509 3f3cb479 bf04ee6a be5dc2c7 3e0e4064 3f107d9d bf2f67ee bec1f57c bd109066 41c80000 42c60000 42240000 358637bd 3e4ccccd 3c23d70a x612062 0 null 1";

#[test]
fn fs505_fs506_a_physical_voxel_array_changed_to_ras() {
    check("FS-506", true, LTACHG_PHYSVOX);
    check("FS-505", true, LTACHG_PHYSVOX);
    let (fix, _) = run(LTACHG_PHYSVOX, UpstreamDefects::Fix);
    let (rep, _) = run(LTACHG_PHYSVOX, UpstreamDefects::Reproduce);
    // the array of type 1; the inverse's matrix (after its centre and spread) none with reproduce
    assert!(fix.starts_with("self 1 1 3f800000 x "), "{fix}");
    assert!(rep.starts_with("self 1 1 3f800000 x "), "{rep}");
    let inv = " 00000000 00000000 00000000 00000000 null null null 0 0 ";
    assert!(rep.contains(inv), "{rep}");
    assert!(!fix.contains(inv), "{fix}");
}

/// §54 (`vrules10`'s lines): a target the RAS-to-RAS type does not support ends the program
/// (status 252) with its message, also for an array of no transforms; an array of another type
/// keeps its type; an array of no transforms takes a supported target and nothing else. The same
/// in both modes.
#[test]
fn lta_change_type_targets_and_other_types() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, _) = run("ltachg 0 1 3f800000 x  5", mode);
        assert!(r.starts_with("EXIT 252 | x "), "{r}");
        assert!(
            r.contains("error: LTAchangeType: you are requesting ras-to-ras to 5 "),
            "{r}"
        );
        let (r, _) = run("ltachg 0 2 3f800000 x  -1", mode);
        assert!(
            r.contains("LTAchangeType: you are requesting physvox-to-physvox to -1 "),
            "{r}"
        );
        let (r, _) = run("ltachg 0 10 3f800000 x  1", mode);
        assert!(r.starts_with("self 0 10 3f800000 x |"), "{r}");
        let (r, _) = run("ltachg 0 0 3f800000 x  2", mode);
        assert!(r.starts_with("self 0 2 3f800000 x |"), "{r}");
    }
}

// ---- part 7b (§34, §35)

/// FS-465: a slice of 3 x 2 floats copied into a given image of one byte (`MRItoImage`): only the
/// byte inside is written, in both modes (FreeSurfer writes past the image's pixels), noted.
#[test]
fn fs465_an_image_too_small_for_the_slice() {
    let line = format!(
        "toimg {} 0 1 1 0 1 3f800000 3f800000 -1 -1 -1 -1 -1 -1 0",
        w("3 2 1 3 1 0", "t3")
    );
    check("FS-465", false, &line);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    assert!(
        fix.starts_with("given 1 1 1 1 0 1 3f800000 3f800000 x"),
        "{fix}"
    );
}

/// FS-466: an RGB image of two frames read into a volume (`ImageToMRI`): by default the second
/// frame from its own pixels, with `reproduce` one byte per pixel from the start, noted in both
/// modes.
#[test]
fn fs466_the_frames_of_an_rgb_image() {
    check(
        "FS-466",
        true,
        "img2mri 2 3 35 2 3f800000 3f800000 17 -1 -1 -1 -1 -1",
    );
}

/// FS-467: negative labels among the neighbours of the parcellation's smoothing: counted like any
/// other label in both modes (FreeSurfer counts them outside its array), noted.
#[test]
fn fs467_negative_labels_in_the_smoothing() {
    for k in [1, 2, 25] {
        let line = format!(
            "smpar {} 3 1 1 1 bf800000 0 0 0 bf800000 2 2 2 c0000000 {k}",
            w("3 3 3 4 1 0", "-1")
        );
        check("FS-467", false, &line);
    }
}

/// FS-468: T1 voxels outside 0 to 255 and NaN clipped as bytes (`MRIclipBrightWM`): by default
/// clamped, with `reproduce` the low byte of their truncation, noted in both modes.
#[test]
fn fs468_values_outside_the_byte_range() {
    let line = format!(
        "clipwm {} {} 0",
        w("8 1 1 3 1 0", "t0"),
        w("8 1 1 0 1 0", "-1")
    );
    check("FS-468", true, &line);
}

/// §35: `MRIcomputeLabelNbhd` with values and no means counts the labels and reads no value
/// (FreeSurfer crashes there); its label count is the arrays' (FreeSurfer's negative count
/// clears an enormous array).
#[test]
fn label_nbhd_values_without_means() {
    let mut p = Process::silent(Environment::new(), UpstreamDefects::Fix);
    let labels = bytes(&mut p, 1, |_| 2.0);
    let vals = bytes(&mut p, 1, |_| 5.0);
    let mut counts = [-7i32; 4];
    let r = p
        .mri_compute_label_nbhd(&labels, Some(&vals), (0, 0, 0), &mut counts, None, 1)
        .expect("counted");
    assert_eq!(r, 0);
    // the 27 places of the window all clamp to the one voxel, of label 2
    assert_eq!(counts, [0, 0, 27, 0]);
    let mut means = [7.0f32; 4];
    let mut none: [i32; 0] = [];
    p.mri_compute_label_nbhd(
        &labels,
        Some(&vals),
        (0, 0, 0),
        &mut none,
        Some(&mut means),
        1,
    )
    .expect("counted");
    // no label is counted with an empty array; the means past it are untouched
    assert_eq!(means, [7.0; 4]);
    p.mri_compute_label_nbhd(
        &labels,
        Some(&vals),
        (0, 0, 0),
        &mut counts,
        Some(&mut means),
        0,
    )
    .expect("counted");
    assert_eq!(counts, [0, 0, 1, 0]);
    assert_eq!(means, [0.0, 0.0, 5.0, 0.0]);
}

// ---- part 8 (§37 to §39)

/// Labels 1, 5, 2 (5 outside a table of 4) and values 10, 50, 60 along a row of three floats.
const PV_LABELS: &str = "i1065353216,1084227584,1073741824";
const PV_VALUES: &str = "i1092616192,1112014848,1114636288";

/// FS-469: a border voxel whose own label lies outside the label table weighed with its mean over
/// the wide window (FreeSurfer reads it outside its table), both modes, noted.
#[test]
fn fs469_an_own_label_outside_the_table() {
    let line = format!(
        "pvlab {} {} 2 4 null null null",
        wp("3 1 1 3 1 0", PV_LABELS, "0"),
        wp("3 1 1 3 1 0", PV_VALUES, "0")
    );
    check("FS-469", false, &line);
}

/// FS-470 and FS-471: no intensity volume at a border voxel, and a negative label limit, end the
/// program in both modes (FreeSurfer crashes), noted.
#[test]
fn fs470_fs471_the_partial_volume_counts_ends() {
    let line = format!(
        "pvlab {} null 2 4 null null null",
        wp("3 1 1 3 1 0", PV_LABELS, "0")
    );
    check("FS-470", true, &line);
    let (r, _) = run(&line, UpstreamDefects::Fix);
    assert!(r.starts_with("EXIT 252 |"), "{r}");
    let line = format!(
        "pvlab {} {} -5 -3 null null null",
        wp("3 1 1 3 1 0", "i3231711232,1065353216,1065353216", "0"),
        wp("3 1 1 3 1 0", PV_VALUES, "0")
    );
    check("FS-471", true, &line);
}

/// The partial-volume count of §37's observed case: labels 1, 2, 2 with values 10, 50, 60 give
/// 1.8205128 for label 2 (the border voxel weighed 40 / 48.75), the mixing volume 0.8205128 there.
#[test]
fn partial_volume_count_of_a_row() {
    let line = format!(
        "pvlab {} {} 2 4 {} null null",
        wp("3 1 1 3 1 0", "i1065353216,1073741824,1073741824", "0"),
        wp("3 1 1 3 1 0", PV_VALUES, "0"),
        w("3 1 1 3 1 0", "0")
    );
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, _) = run(&line, mode);
        let v = f32::from_bits(u32::from_str_radix(&r[..8], 16).expect("hex"));
        assert!((v - 1.820_512_8).abs() < 1e-6, "{r}");
    }
}

/// FS-472: 300 into a byte volume (`MRIconst`): its low bits (44) with `reproduce`, 255 by
/// default.
#[test]
fn fs472_a_constant_past_the_byte_range() {
    let line = format!("const 1 1 1 1 43960000 {}", w("1 1 1 0 1 0", "0"));
    check("FS-472", true, &line);
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    let (fix, _) = run(&line, UpstreamDefects::Fix);
    assert!(rep.starts_with("given ") && rep.contains(" x2c |"), "{rep}");
    assert!(fix.starts_with("given ") && fix.contains(" xff |"), "{fix}");
}

/// FS-473: b-tables of one row for two frames (`MRIreverseSliceOrder`): the second row left 0 in
/// both modes (FreeSurfer reads past the table), noted.
#[test]
fn fs473_a_b_table_shorter_than_the_frames() {
    let line = format!(
        "revsl {} 1 1 3f800000 1 3 3f800000 40000000 40400000 0 new",
        wp("1 1 2 3 2 0", "0", "0")
    );
    check("FS-473", false, &line);
}

/// FS-474: values 300.7, 2 and -1.5 ordered as bytes (`MRIorderIndices`): 2, 44, 255 with
/// `reproduce` (the coordinates 1, 0, 2), clamped by default (-1.5 first).
#[test]
fn fs474_an_order_of_values_past_a_byte() {
    let line = format!(
        "ordidx {} 0",
        wp("3 1 1 3 1 0", "i1133926810,1073741824,3217031168", "0")
    );
    check("FS-474", true, &line);
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    assert!(rep.starts_with("0 1 0 2 0 0 0 0 0 0 |"), "{rep}");
}

/// The volume fractions of a row of two segmentation voxels doubled onto three (`vfrac`): the
/// middle voxel, which no segmentation voxel reaches, maps back onto the second.
fn vfrac_line(labels: &str) -> String {
    format!(
        "vfrac {} 4 4 40000000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 \
         00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 {} {}",
        w("3 1 1 3 1 0", "0"),
        wp("2 1 1 3 1 0", labels, "0"),
        w("3 1 1 3 2 0", "0")
    )
}

/// FS-475 and FS-476: label 0 left out in both modes (FreeSurfer counts it outside the volume);
/// the last label marked in the second pass by default, not with `reproduce`.
#[test]
fn fs475_fs476_the_volume_fractions_labels() {
    check("FS-475", false, &vfrac_line("i0,1065353216"));
    check("FS-476", true, &vfrac_line("i1073741824"));
}

/// FS-477: an index equal to the extent (`MRIapplyDiffVect`): written with `reproduce` (past the
/// volume), refused by default; FS-478: in place with a colour table, the table lost with
/// `reproduce`, kept by default.
#[test]
fn fs477_fs478_the_difference_lists() {
    let row =
        "4000000000000000 0000000000000000 0000000000000000 0000000000000000 4014000000000000";
    let line = format!("applyv {} null 1 {row} new", wp("2 1 1 3 1 0", "0", "0"));
    check("FS-477", true, &line);
    let ct = "null 1 x 1 0 x 1 x41 10 20 30 0 3f000000 3f000000 3f000000 3f800000 1 3 null";
    let wct = w("2 1 1 3 1 0", "0").replace(" null null", &format!(" {ct}"));
    let line = format!("applyv {wct} 0 null 0 self");
    check("FS-478", true, &line);
}

/// FS-479: a transform file of no linear transform (`MRIestimateTIV`) ends the program in both
/// modes (FreeSurfer crashes), noted.
#[test]
fn fs479_a_transform_file_of_no_transform() {
    use larmorx_fs_core::mri::VolGeom;
    use larmorx_fs_core::volume::ltaread::TransformInputs;
    struct One(Vec<u8>);
    impl TransformInputs for One {
        fn read_file(&mut self, _name: &[u8]) -> Result<Vec<u8>, i32> {
            Ok(self.0.clone())
        }
        fn volume_geometry(
            &mut self,
            _p: &mut Process,
            _name: &[u8],
        ) -> Result<Option<VolGeom>, i32> {
            Err(2)
        }
    }
    #[derive(Clone, Default)]
    struct ErrBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for ErrBuf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("buffer").extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let e = ErrBuf::default();
        let mut p = Process::new(
            Environment::new(),
            mode,
            Box::new(std::io::sink()),
            Box::new(e.clone()),
        );
        let mut input = One(b"type = 1\nnxforms = 0\n".to_vec());
        let r = p.mri_estimate_tiv(b"talairach.lta", 1755.0, None, &mut input);
        assert!(r.is_err(), "the end of the program");
        assert!(noted(&mut p, "FS-479"));
        p.flush();
        let err = String::from_utf8_lossy(&e.0.lock().expect("buffer")).into_owned();
        assert!(
            err.contains(
                "MRIestimateTIV(): talairach.lta holds no linear transform (FreeSurfer crashes here)\n"
            ),
            "{err}"
        );
    }
}

/// §37's FS-471 limits (`vrules13`): -1 to -3 count a volume of the label only (no border voxel)
/// as usual; from -4 the first counted voxel ends the program (status 252); and FS-470's error
/// names the border voxel. Both modes.
#[test]
fn fs470_fs471_the_errors_and_the_limits_off_the_border() {
    // three voxels of label -200 (no border voxel), values 10, 50, 60
    let labels = wp("3 1 1 3 1 0", "i3276275712", "0");
    let vals = wp("3 1 1 3 1 0", PV_VALUES, "0");
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        for m in [-1, -2, -3] {
            let (r, _) = run(
                &format!("pvlab {labels} {vals} -200 {m} null null null"),
                mode,
            );
            assert!(r.starts_with("40400000 "), "{m}: {r}");
        }
        for m in [-4, -17] {
            let (r, ids) = run(
                &format!("pvlab {labels} {vals} -200 {m} null null null"),
                mode,
            );
            assert!(r.starts_with("EXIT 252 |"), "{m}: {r}");
            assert!(
                r.contains(&format!(
                    "MRIvoxelsInLabelWithPartialVolumeEffects(): a negative maximum label number {m} (FreeSurfer crashes here)\n"
                )),
                "{r}"
            );
            assert!(ids.iter().any(|i| i == "FS-471"), "{ids:?}");
        }
        let line = format!(
            "pvlab {} null 2 4 null null null",
            wp("3 1 1 3 1 0", PV_LABELS, "0")
        );
        let (r, _) = run(&line, mode);
        assert!(
            r.contains(
                "MRIvoxelsInLabelWithPartialVolumeEffects(): no intensity volume for the border voxel ("
            ) && r.contains(") (FreeSurfer crashes here)\n"),
            "{r}"
        );
    }
}

/// FS-266 (§39): fewer powers than frames (`mri_framepower`) end the program before any frame
/// changes (rule B, status 252), both modes, noted.
#[test]
fn fs266_framepower_with_fewer_powers_than_frames() {
    let line = format!("fpow {} 1 40000000", wp("1 1 1 3 2 0", "i1073741824", "0"));
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(&line, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(
            r.contains("mri_framepower(): fewer powers than frames (FreeSurfer reads past them)\n"),
            "{r}"
        );
        assert!(ids.iter().any(|i| i == "FS-266"), "{ids:?}");
    }
    // as many powers as frames: not noted
    let line = format!(
        "fpow {} 2 40000000 3f800000",
        wp("1 1 1 3 2 0", "i1073741824", "0")
    );
    let (_, ids) = run(&line, UpstreamDefects::Fix);
    assert!(!ids.iter().any(|i| i == "FS-266"), "{ids:?}");
}

/// FS-480: frames -1, 0, 0 with zeros excluded (`MRIvote`): the last run of zeros taken with
/// `reproduce` (0, 2/3), left out by default (-1, 1/3).
#[test]
fn fs480_a_last_run_of_zeros() {
    let line = format!(
        "vote {} null new 1",
        wp("1 1 1 3 3 0", "i3212836864,0,0", "0")
    );
    check("FS-480", true, &line);
}

/// FS-481: a reshape of 64-bit voxels: 32-bit halves at their indices with `reproduce`, whole
/// voxels by default.
#[test]
fn fs481_a_reshape_of_64_bit_voxels() {
    let line = format!("reshape {} 1 2 1 1", wp("2 1 1 2 1 0", "7", "0"));
    check("FS-481", true, &line);
}

// ---- §54: the rest of the operations on a transform array (FS-504, FS-507, FS-508)

/// G1 to G3 (Gf): three valid geometries; M1, M2 invertible matrices, SING a singular one.
const G1: &str = "1 4 3 2 40000000 3fc00000 3f000000 1 00000000 3f800000 00000000 bf800000 00000000 00000000 00000000 00000000 3f800000 41200000 c0a00000 40400000 00000000 00000000 00000000 x 1 null";
const G2: &str = "1 3 5 2 3f800000 40400000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 bf800000 40000000 40e00000 00000000 00000000 00000000 x 1 null";
const G3: &str = "1 2 2 6 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 x 1 null";
const M1: &str = "4 4 3f8ccccd 3e4ccccd 00000000 40a00000 bdcccccd 3f666666 3e99999a c0000000 00000000 3dcccccd 3f99999a 40800000 00000000 00000000 00000000 3f800000";
const M2: &str = "4 4 00000000 bf800000 00000000 41480000 3f800000 00000000 00000000 c0400000 00000000 00000000 40000000 3f400000 00000000 00000000 00000000 3f800000";
const SING: &str = "4 4 3f800000 40000000 00000000 00000000 40000000 40800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000";
const IDENT: &str = "4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000";

/// An array (LTA) of type `ty`: the transforms (matrix, source, destination), their inverses
/// the identity between the same geometries the other way round.
fn arr54(ty: i32, items: &[(&str, &str, &str)]) -> String {
    let z = format!("4 4 {}", ["00000000"; 16].join(" "));
    let lt = |m: &str, a: &str, b: &str| {
        format!("00000000 00000000 00000000 3f800000 {m} {z} {z} 0 0 {a} {b}")
    };
    let fw: Vec<String> = items.iter().map(|(m, a, b)| lt(m, a, b)).collect();
    let inv: Vec<String> = items.iter().map(|(_, a, b)| lt(IDENT, b, a)).collect();
    format!(
        "{} {ty} 3f800000 x {} {}",
        items.len(),
        fw.join(" "),
        inv.join(" ")
    )
}

/// The standard output of a result line (`<result> | x<out> <err>`) decoded.
fn out_of(r: &str) -> String {
    let (_, streams) = r.rsplit_once(" | ").expect("a result line");
    let out = streams.split(' ').next().unwrap_or("x");
    let b: Vec<u8> = (1..out.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&out[i..i + 2], 16).expect("hex"))
        .collect();
    String::from_utf8_lossy(&b).into_owned()
}

/// FS-507: REGISTER_DAT to RAS-to-RAS of two transforms: with `reproduce` the first transform
/// changed twice and the second left; by default each changed once; noted in both modes.
#[test]
fn fs507_register_dat_to_ras_of_two_transforms() {
    let line = format!("ltachg {} 1", arr54(14, &[(M1, G1, G2), (M2, G2, G3)]));
    check("FS-507", true, &line);
    let (rep, _) = run(&line, UpstreamDefects::Reproduce);
    // the second transform's matrix is still M2's under reproduce
    assert!(rep.contains(&M2[4..]), "{rep}");
    let one = format!("ltachg {} 1", arr54(14, &[(M1, G1, G2)]));
    let (a, ida) = run(&one, UpstreamDefects::Fix);
    let (b, idb) = run(&one, UpstreamDefects::Reproduce);
    assert_eq!(a, b);
    assert!(
        !ida.iter().chain(&idb).any(|i| i == "FS-507"),
        "one transform: not noted"
    );
}

/// FS-508: a REGISTER_DAT registration that cannot be inverted changed to FSL: the program ends
/// in both modes (status 252), noted.
#[test]
fn fs508_an_fsl_conversion_without_a_matrix() {
    let line = format!(
        "+env=FSLOUTPUTTYPE=NIFTI_GZ ltachg {} 15",
        arr54(14, &[(SING, G1, G2)])
    );
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(&line, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(r.contains("LTAchangeType: the FSL conversion gave no matrix (FreeSurfer frees the transform's matrix and goes on using it here)\n"), "{r}");
        assert!(ids.iter().any(|i| i == "FS-508"), "{ids:?}");
    }
}

/// FS-508's place (§54, relay 15's answer): the program ends at the first read of the freed
/// matrix, after the later transforms' conversions: the diagnostic print of the first transform
/// (after `transformed matrix:`), else the inverses' fill at that transform (the earlier
/// transforms filled and, at the diagnostic level 1, the first one's matrix printed first).
#[test]
fn fs508_ends_at_the_first_read_of_the_freed_matrix() {
    let fsl = "+env=FSLOUTPUTTYPE=NIFTI_GZ";
    let first = arr54(14, &[(SING, G1, G2), (M1, G2, G3)]);
    let second = arr54(14, &[(M1, G1, G2), (SING, G2, G3)]);
    let ends = |line: &str, mode| {
        let (r, ids) = run(line, mode);
        assert!(r.starts_with("EXIT 252 |"), "{line}: {r}");
        assert!(r.contains("LTAchangeType: the FSL conversion gave no matrix (FreeSurfer frees the transform's matrix and goes on using it here)\n"), "{r}");
        assert!(ids.iter().any(|i| i == "FS-508"), "{ids:?}");
        out_of(&r)
    };
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        // the first transform's freed matrix read by the print, after both conversions' lines:
        // nothing after the print's first line
        let out = ends(&format!("{fsl} +gdiagno=1 ltachg {first} 15"), mode);
        assert_eq!(out.matches("tkreg2FSL: mov det").count(), 2, "{out}");
        assert!(
            out.ends_with("registration matrix.\ntransformed matrix:\n"),
            "{out}"
        );
        // the second's read by the fill, the first one's matrix printed before it
        let out = ends(&format!("{fsl} +gdiagno=1 ltachg {second} 15"), mode);
        let (_, after) = out.split_once("transformed matrix:\n").expect("the print");
        assert_eq!(after.lines().count(), 4, "{out}");
        // at level 0 both end in the fill, nothing printed by the change itself
        for a in [&first, &second] {
            ends(&format!("{fsl} ltachg {a} 15"), mode);
        }
    }
    // the second transform's own end comes first: a geometry whose K has no inverse (a voxel size
    // of 0) as its source ends the program in its own conversion, nothing noted
    let g0 = G3.replacen(
        "3f800000 3f800000 3f800000 1",
        "00000000 3f800000 3f800000 1",
        1,
    );
    let line = format!(
        "{fsl} ltachg {} 15",
        arr54(14, &[(SING, G1, G2), (M1, &g0, G3)])
    );
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(&line, mode);
        assert!(!r.contains("the FSL conversion gave no matrix"), "{r}");
        assert!(!ids.iter().any(|i| i == "FS-508"), "{ids:?}");
    }
}

/// FS-504: a transform the array does not hold read (no arrays, an array without transforms, a
/// matrix smaller than 4 x 4, the diagnostic print of an empty array): the program ends in both
/// modes (status 252), noted.
#[test]
fn fs504_transforms_an_array_does_not_hold() {
    let empty = arr54(1, &[]);
    let lines = [
        ("ltacat 0 0".to_owned(), "LTAconcat: no transform arrays"),
        (
            format!("ltacat 2 {} {empty} 0", arr54(1, &[(M1, G1, G2)])),
            "LTAconcat: a transform array without transforms",
        ),
        (
            format!("ltacat 2 {empty} {} 0", arr54(1, &[(M1, G1, G2)])),
            "LTAconcat: a transform array without transforms",
        ),
        (
            format!("ltared {empty}"),
            "LTAreduce: a transform array without transforms",
        ),
        (
            format!("ltasrc {empty} {}", w("1 1 1 0 1 0", "0")),
            "LTAmriIsSource: a transform array without transforms",
        ),
        (
            format!(
                "ltaprt {}",
                arr54(
                    1,
                    &[(
                        "3 4 3f800000 40000000 40400000 40800000 40a00000 40c00000 40e00000 41000000 41100000 41200000 41300000 41400000",
                        G1,
                        G2
                    )]
                )
            ),
            "LTAprint: a transform without a 4 x 4 matrix",
        ),
        (
            format!("+gdiagno=1 ltachg {} 1", arr54(14, &[])),
            "LTAchangeType: a transform array without transforms",
        ),
    ];
    for (line, why) in &lines {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (r, ids) = run(line, mode);
            assert!(r.starts_with("EXIT 252 |"), "{line}: {r}");
            assert!(
                r.contains(&format!(
                    "{why} (FreeSurfer reads memory it does not own here)\n"
                )),
                "{line}: {r}"
            );
            assert!(ids.iter().any(|i| i == "FS-504"), "{line}: {ids:?}");
        }
    }
    // the print's lines up to the short matrix's 1 4 4, and the diagnostic print's first line
    let (r, _) = run(&lines[5].0, UpstreamDefects::Fix);
    assert!(out_of(&r).ends_with("sigma     = 1.0000\n1 4 4\n"), "{r}");
    let (r, _) = run(&lines[6].0, UpstreamDefects::Fix);
    assert_eq!(out_of(&r), "transformed matrix:\n");
}

/// §54: the targets REGISTER_DAT, FSL and coronal RAS do not support end the program (status 252)
/// with their messages, also for an array of no transforms; the diagnostic print of a change from
/// REGISTER_DAT; a NaN element printed as the C library prints it.
#[test]
fn transform_array_targets_prints_and_nan_elements() {
    for (ty, tg, name) in [
        (14, 2, "REGISTER_DAT"),
        (15, -1, "FSLREG_TYPE"),
        (21, 0, "COR_RAS_TO_COR_RAS"),
    ] {
        for items in [&[(M1, G1, G2)][..], &[][..]] {
            let line = format!("ltachg {} {tg}", arr54(ty, items));
            let (r, _) = run(&line, UpstreamDefects::Fix);
            assert!(r.starts_with("EXIT 252 |"), "{r}");
            assert!(
                r.contains(&format!(
                    "LTAchangeType unsupported: you are requesting {name} to {tg} "
                )),
                "{r}"
            );
        }
    }
    let line = format!("+gdiagno=1 ltachg {} 1", arr54(14, &[(M1, G1, G2)]));
    let (r, _) = run(&line, UpstreamDefects::Fix);
    let out = out_of(&r);
    assert!(
        out.starts_with("transformed matrix:\n") && out.ends_with(";\n"),
        "{out}"
    );
    assert_eq!(out.lines().count(), 5, "{out}");
    let nanm = "4 4 3f800000 00000000 00000000 7fc00000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000";
    let (r, _) = run(
        &format!("ltaprt {}", arr54(1, &[(nanm, G1, G2)])),
        UpstreamDefects::Fix,
    );
    assert!(
        out_of(&r).contains("0.000000000000000e+00                nan \n"),
        "{}",
        out_of(&r)
    );
}

/// §54's `LTAreduce` as its text reads: each next transform compared with the previous
/// transform's own destination. G1 to G2, then G3 to G2 (taken inverted: a warning), then G3 to G1:
/// its source G3 is not the previous transform's destination G2, nor is its destination: the
/// mismatch (reading the chain's end instead would have matched G3). The probe's groups do not
/// tell the two readings apart; asked of the spec writer (relay 15).
#[test]
fn lta_reduce_compares_with_the_previous_transforms_destination() {
    let line = format!(
        "ltared {}",
        arr54(1, &[(M1, G1, G2), (M2, G3, G2), (M1, G3, G1)])
    );
    let (r, _) = run(&line, UpstreamDefects::Reproduce);
    assert!(r.starts_with("null "), "{r}");
    assert_eq!(
        out_of(&r),
        "WARNING: LTAreduce(): inverting LT 1 to match geometry\nERROR: LTAreduce(): LTs 1 and 2 do not match\n"
    );
}

// ---- §55 to §57's defects (FS-267, FS-509, FS-510, FS-512 to FS-517)

/// vrules15 line 11.
const R15_FS510: &str = "a2v 4 1 1 1 1 0 i1,1,2,3 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 2 1 1 1 1 0 i0 40800000 41000000 41000000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0000000000000000 4 x6c7574 0 0 x 1 x6c616230 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616231 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616232 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616233 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 2 x74 0 0 x 1 x7479706530 0 0 0 0 00000000 00000000 00000000 00000000 0 0 1 x7479706531 0 0 0 0 00000000 00000000 00000000 00000000 0 0 null";
/// vrules15 line 15.
const R15_FS267: &str = "a2v 4 1 1 1 1 0 i1,2,3,4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 2 1 1 1 1 0 i0 40800000 41000000 41000000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0000000000000000 5 x6c7574 0 0 x 1 x6c616230 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616231 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616232 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616233 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616234 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 0 x74 0 0 x null";
/// vrules15 line 17.
const R15_FS509_PAST: &str = "a2v 4 1 1 1 1 0 i1,2,3,4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 2 1 1 1 1 0 i0 40800000 41000000 41000000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0000000000000000 4 x6c7574 0 0 x 1 x6c616230 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616231 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616232 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616233 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 2 x74 0 0 x 1 x7479706530 0 0 0 0 00000000 00000000 00000000 00000000 0 0 1 x7479706531 0 0 0 0 00000000 00000000 00000000 00000000 0 0 null";
/// vrules15 line 18.
const R15_FS509_EMPTY: &str = "a2v 4 1 1 1 1 0 i1,2,3,4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 2 1 1 1 1 0 i0 40800000 41000000 41000000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0000000000000000 5 x6c7574 0 0 x 1 x6c616230 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616231 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 0 1 x6c616233 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616234 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 2 x74 0 0 x 1 x7479706530 0 0 0 0 00000000 00000000 00000000 00000000 0 0 1 x7479706531 0 0 0 0 00000000 00000000 00000000 00000000 0 0 null";
/// vrules15 line 19.
const R15_FS509_TYPE: &str = "a2v 4 1 1 1 1 0 i1,2,3,4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 2 1 1 1 1 0 i0 40800000 41000000 41000000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0000000000000000 5 x6c7574 0 0 x 1 x6c616230 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616231 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616232 1 2 3 0 3f000000 3f000000 3f000000 3f000000 100 0 1 x6c616233 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616234 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 2 x74 0 0 x 1 x7479706530 0 0 0 0 00000000 00000000 00000000 00000000 0 0 1 x7479706531 0 0 0 0 00000000 00000000 00000000 00000000 0 0 null";
/// vrules15 line 35.
const R15_FS512: &str = "zcat 2 3 1 1 1 0 i0,1,2,3,4,5 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 3 2 1 1 0 i10,11,12,13,14,15,16,17,18,19,20,21 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 new";
/// vrules15 line 39.
const R15_FS513: &str = "shdim 2 4 4 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 1 1";
/// vrules15 line 47.
const R15_FS514: &str = "lacj 5 3 3 1 1 0 i3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,18,3,54,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 new";
/// vrules15 line 57.
const R15_FS517: &str = "ccrs 4 4 2 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 41dfffffffc00000 3ff0000000000000 0000000000000000 3 1 1 0";
/// vrules15 line 61.
const R15_FS515: &str = "crras 4 4 2 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 3f800000 3f800000 3f800000 1 3f4ccccd 3f19999a 00000000 3f400000 bf28f5c3 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 3ff0000000000000 3ff0000000000000 0000000000000000 2 2 1 0 null";
/// vrules15 line 62.
const R15_FS516: &str = "crras 4 4 2 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 3ff0000000000000 3ff0000000000000 0000000000000000 2 2 1 0 1 1 3f800000 x 410254e0 bb98842b 28698c87 3f000000 4 4 3f74b7b7 3dc560b3 3c640ea5 3d110bb2 3dcbfc1f 3f69000e bd13912f be9184a1 3d9f3ba2 bcf955a4 3f68469b bed0db23 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 4 4 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 00000000 00000000 00000000 00000000 00000000 x 1 null 1 3 3 3 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 c0000000 3f800000 00000000 00000000 00000000 00000000 x 1 null c2b1dfab c299549d c2b16afc 461c4000 4 4 3f6508f8 3d8961c7 bdb2890a c175e0d3 bddb7589 3f971dca 3f872eea c296c4a0 bf421621 bfb649dd 3e4ab0a8 41dae810 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 1 0 1 3 3 3 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 c0000000 3f800000 00000000 00000000 00000000 00000000 x 1 null 1 4 4 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 00000000 00000000 00000000 00000000 00000000 x 1 null";

/// A vote choosing tissue type 1 (line 12's call with the types 0, 1, 1, 1, 0), then line 15's
/// table of no places in the same process (`a2vtwice`, the candidate's own command).
const R15_FS267_TWICE: &str = "a2vtwice 4 1 1 1 1 0 i1,2,3,4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 2 1 1 1 1 0 i0 40800000 41000000 41000000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0000000000000000 5 x6c7574 0 0 x 1 x6c616230 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616231 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616232 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616233 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616234 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 2 x74 0 0 x 1 x7479706530 0 0 0 0 00000000 00000000 00000000 00000000 0 0 1 x7479706531 0 0 0 0 00000000 00000000 00000000 00000000 0 0 null 4 1 1 1 1 0 i1,2,3,4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 2 1 1 1 1 0 i0 40800000 41000000 41000000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0000000000000000 5 x6c7574 0 0 x 1 x6c616230 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616231 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616232 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 1 x6c616233 1 2 3 0 3f000000 3f000000 3f000000 3f000000 1 0 1 x6c616234 1 2 3 0 3f000000 3f000000 3f000000 3f000000 0 0 0 x74 0 0 x null";

/// FS-510 (`vrules15`): labels 1, 1, 2, 3 of the types 1, 0, 0: FreeSurfer votes (its highest
/// label's quarter) and writes 3; by default 1 wins with half the hits.
#[test]
fn fs510_the_clear_winners_test() {
    check("FS-510", true, R15_FS510);
}

/// FS-509 (`vrules15`): a label past the table, an empty place and a tissue type of 100 end the
/// program in both modes (status 252), noted.
#[test]
fn fs509_labels_and_types_past_the_tables() {
    for (line, why) in [
        (R15_FS509_PAST, "the label 4 is not in the colour table"),
        (R15_FS509_EMPTY, "the label 2 is not in the colour table"),
        (
            R15_FS509_TYPE,
            "the tissue type 100 of the label 2 (100 or more)",
        ),
    ] {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (r, ids) = run(line, mode);
            assert!(r.starts_with("EXIT 252 |"), "{r}");
            assert!(
                r.contains(&format!(
                    "MRIaseg2vol(): {why} (FreeSurfer reads past its tables here)\n"
                )),
                "{r}"
            );
            assert!(ids.iter().any(|i| i == "FS-509"), "{ids:?}");
        }
    }
}

/// FS-267: a table of no places takes the type 0 in a program's first vote (line 15: both modes
/// alike, noted); after a vote that chose type 1, `reproduce` takes 1 (the label of type 1, 4)
/// and the default 0 (the highest label of type 0, 5... of those present).
#[test]
fn fs267_a_tissue_table_of_no_places() {
    check("FS-267", false, R15_FS267);
    let (fix, ids) = run(R15_FS267_TWICE, UpstreamDefects::Fix);
    let (rep, rids) = run(R15_FS267_TWICE, UpstreamDefects::Reproduce);
    assert!(ids.iter().chain(&rids).any(|i| i == "FS-267"), "{ids:?}");
    assert_ne!(fix, rep);
}

/// FS-512 (`vrules15`): two volumes 2 wide and 3 high concatenated: FreeSurfer copies 2 rows
/// (the third stays 0), by default all 3.
#[test]
fn fs512_the_rows_concatenated() {
    check("FS-512", true, R15_FS512);
}

/// FS-513 (`vrules15`): rows of a 2 x 4 x 4 volume shifted by 1 and wrapped: by the width (2) in
/// FreeSurfer, by the height (4) by default.
#[test]
fn fs513_the_rows_wrapped() {
    check("FS-513", true, R15_FS513);
}

/// FS-514 (`vrules15`): a cortex voxel next to both amygdalae: the later write stays (7031) in
/// both modes, noted.
#[test]
fn fs514_a_cortex_voxel_next_to_both_amygdalae() {
    check("FS-514", false, R15_FS514);
}

/// FS-515 (`vrules15`): an orientation naming x twice: `reproduce` crops to the field of view
/// never set as 0 (FreeSurfer's memory contents in its runs), the default ends the program.
#[test]
fn fs515_an_orientation_naming_an_axis_twice() {
    check("FS-515", true, R15_FS515);
    let (r, _) = run(R15_FS515, UpstreamDefects::Fix);
    assert!(r.starts_with("EXIT 252 |"), "{r}");
}

/// FS-516 (`vrules15`): a volume not marked valid, the array from it: by default inverted (the
/// volume is its source), with `reproduce` taken as it is (it equals no geometry).
#[test]
fn fs516_the_arrays_direction() {
    check("FS-516", true, R15_FS516);
    let (fix, _) = run(R15_FS516, UpstreamDefects::Fix);
    assert!(out_of(&fix).starts_with("Inverting LTA\n"), "{fix}");
}

/// FS-517 (`vrules15`): a centre at `int`'s top with a field of view of 3: the program ends in both
/// modes (status 252), noted.
#[test]
fn fs517_a_last_voxel_beyond_int() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R15_FS517, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(ids.iter().any(|i| i == "FS-517"), "{ids:?}");
    }
}

// ---- items 85 to 88 (vrules16): FS-268, FS-515 under reproduce, FS-517 at int's maximum, FS-511

/// vrules16 line 22.
const R16_RAS: &str = "crras 6 5 4 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 3fd3333333333333 bfc999999999999a 3fb999999999999a 4 1 3 0 null";
/// vrules16 line 30.
const R16_SRA: &str = "crras 6 5 4 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119 3f800000 3f800000 3f800000 1 00000000 00000000 3f800000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 3fd3333333333333 bfc999999999999a 3fb999999999999a 4 1 3 0 null";
/// vrules16 line 41.
const R16_RRS: &str = "crras 4 4 2 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 3f800000 3f800000 3f800000 1 3f4ccccd 3f19999a 00000000 3f400000 bf28f5c3 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 3ff0000000000000 3ff0000000000000 0000000000000000 4 1 3 0 null";
/// vrules16 line 44.
const R16_FS517_MAX: &str = "ccrs 4 4 2 1 1 0 i0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 41dfffffffc00000 3ff0000000000000 3ff0000000000000 1 1 1 0";
/// vrules16 line 60.
const R16_FS511_TENSOR: &str = "spmw 4 4 4 6 1 0 i17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 null 4 3 3 3 3 0 i0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3204448256,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1048576000,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 1 new";
/// vrules16 line 62.
const R16_FS511_RGB: &str = "spmw 4 4 4 9 1 0 i17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 null 4 3 3 3 3 0 i0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3204448256,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1048576000,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 1 new";

/// FS-268 (`vrules16`): `SRA` with `(4, 1, 3)`: by default each voxel axis takes the entry its
/// letter names, `(3, 4, 1)`; with `reproduce` FreeSurfer's `(1, 3, 4)`; noted in both modes.
#[test]
fn fs268_the_fields_of_view_of_a_3_cycle() {
    check("FS-268", true, R16_SRA);
    let (fix, _) = run(R16_SRA, UpstreamDefects::Fix);
    let (rep, _) = run(R16_SRA, UpstreamDefects::Reproduce);
    assert!(out_of(&fix).contains("ostr SRA (4 1 3) (3 4 1)\n"), "{fix}");
    assert!(out_of(&rep).contains("ostr SRA (4 1 3) (1 3 4)\n"), "{rep}");
}

/// FS-268 (`vrules16`): `RAS` keeps every axis in place: the two readings agree, nothing noted.
#[test]
fn fs268_not_noted_where_the_readings_agree() {
    let (fix, ids) = run(R16_RAS, UpstreamDefects::Fix);
    let (rep, _) = run(R16_RAS, UpstreamDefects::Reproduce);
    assert_eq!(fix, rep);
    assert!(!ids.iter().any(|i| i == "FS-268"), "{ids:?}");
}

/// FS-515 (`vrules16`): `RRS` with `(4, 1, 3)`: with `reproduce` the second axis's entry kept on
/// the first voxel axis, the second never set (0), noted; by default the program ends (status
/// 252) with larmorx's error, noted.
#[test]
fn fs515_the_later_axis_kept_and_the_default_end() {
    let (rep, rids) = run(R16_RRS, UpstreamDefects::Reproduce);
    assert!(out_of(&rep).contains("ostr RRS (4 1 3) (1 0 3)\n"), "{rep}");
    assert!(rids.iter().any(|i| i == "FS-515"), "{rids:?}");
    let (fix, fids) = run(R16_RRS, UpstreamDefects::Fix);
    assert!(fix.starts_with("EXIT 252 |"), "{fix}");
    assert!(fids.iter().any(|i| i == "FS-515"), "{fids:?}");
}

/// FS-517 (`vrules16`): a last column at `int`'s maximum (a centre of 2147483647, a field of view
/// of 1): the path outside the volume up to its `c_ras` line, then the end in both modes (status
/// 252), noted.
#[test]
fn fs517_a_last_voxel_at_the_int_maximum() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R16_FS517_MAX, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(out_of(&r).contains("c_ras of new volume "), "{r}");
        assert!(ids.iter().any(|i| i == "FS-517"), "{ids:?}");
    }
}

/// FS-511 (`vrules16`): a trilinear sample of a tensor and of an RGB volume, failing before any
/// value was written: 0 in both modes, noted; the modes' volumes agree (their streams differ by
/// FS-01's NUL after the samplers' errors).
#[test]
fn fs511_a_failing_first_sample() {
    for line in [R16_FS511_TENSOR, R16_FS511_RGB] {
        let (fix, fids) = run(line, UpstreamDefects::Fix);
        let (rep, rids) = run(line, UpstreamDefects::Reproduce);
        assert!(fids.iter().any(|i| i == "FS-511"), "{fids:?}");
        assert!(rids.iter().any(|i| i == "FS-511"), "{rids:?}");
        assert_eq!(fix.split(" | ").next(), rep.split(" | ").next());
    }
}

// ---- part 9 (vrules17 to vrules19): FS-269, FS-270, FS-482, FS-483, FS-484, FS-485, FS-323 in §44

/// vrules18 line 1.
const R18_PAD0: &str = "rgbb 6 5 4 3 1 0 i0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 0";
/// vrules18 line 2.
const R18_PAD1: &str = "rgbb 6 5 4 3 1 0 i0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 1";
/// vrules18 line 21.
const R18_EQODD: &str = "rgbb 6 6 4 3 1 0 i0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,1065353216,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 0";
/// vrules18 line 23.
const R18_WIDE: &str = "rgbb 4 3 3 3 1 0 i1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 0";
/// vrules18 line 25.
const R18_MASKUP: &str = "maskup 4 4 3 3 1 0 i0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,0,1065353216,1073741824,1077936128,1082130432,1084227584 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3 3 1 0 i0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1065353216,1065353216,0,0,1065353216,1065353216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 0 0";
/// vrules17 line 26.
const R17_FS482: &str = "vvsm 3 3 3 3 1 0 i0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,1088421888,1090519040,1091567616,1092616192,1093664768,1094713344,1095761920,1096810496,1097859072,1098907648,1099431936,1099956224,1100480512,1101004800,1101529088,1102053376,1102577664,1103101952,1103626240,1104150528 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0000000000000000 3 3 3 3 1 0 i0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 00000000 3f000000 00000000 3f800000 00000000 bf000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 1 3f800000 3 3 3 6 1 0 i7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0000000000000000 null 1";
/// vrules17 line 33.
const R17_A2VMU: &str = "a2vmu 2 2 2 1 1 0 i0,1,2,1,2,1,0,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f000000 3f000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 1 3f800000 x 00000000 00000000 00000000 3f800000 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 2 2 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f000000 3f000000 00000000 00000000 00000000 00000000 x 1 null 1 4 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 bf000000 00000000 3f000000 00000000 00000000 00000000 x 1 null 00000000 00000000 00000000 3f800000 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 4 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 bf000000 00000000 3f000000 00000000 00000000 00000000 x 1 null 1 2 2 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f000000 3f000000 00000000 00000000 00000000 00000000 x 1 null 0000000000000000 2 null";
/// vrules19 line 0.
const R19_HALF: &str = "cov 2 2 1 3 2 0 i1065353216,1073741824,1077936128,1082130432,1056964608,3212836864,1075838976,1090519040 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 2 1 3 1 0 i1056964608,1056964608,1056964608,1056964608 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0";
/// vrules19 line 2.
const R19_MIXED: &str = "cov 2 2 1 3 2 0 i1065353216,1073741824,1077936128,1082130432,1056964608,3212836864,1075838976,1090519040 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 2 1 3 1 0 i1065353216,1056964608,0,2143289344 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0";
/// vrules19 line 8.
const R19_COV0: &str = "cov 2 2 1 3 0 0 i0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 null";
/// vrules19 line 9.
const R19_PCA0: &str = "mpca 2 2 1 3 0 0 i0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 null";
/// vrules19 line 48.
const R19_VXN500: &str = "vxn 5 5 5 3 1 0 i0,1065353216,1073741824,1077936128,1082130432,1073741824,1077936128,1082130432,1084227584,1086324736,1082130432,1084227584,1086324736,1088421888,1090519040,1086324736,1088421888,1090519040,1091567616,1092616192,1090519040,1091567616,1092616192,1093664768,1094713344,1077936128,1082130432,1084227584,1086324736,1088421888,1084227584,1086324736,1088421888,1090519040,1091567616,1088421888,1090519040,1091567616,1092616192,1093664768,1091567616,1092616192,1093664768,1094713344,1095761920,1093664768,1094713344,1095761920,1096810496,1097859072,1086324736,1088421888,1090519040,1091567616,1092616192,1090519040,1091567616,1092616192,1093664768,1094713344,1092616192,1093664768,1099431936,1095761920,1096810496,1094713344,1095761920,1096810496,1097859072,1098907648,1096810496,1097859072,1098907648,1099431936,1099956224,1091567616,1092616192,1093664768,1094713344,1095761920,1093664768,1094713344,1095761920,1096810496,1097859072,1095761920,1096810496,1097859072,1098907648,1099431936,1097859072,1098907648,1099431936,1099956224,1100480512,1099431936,1099956224,1100480512,1101004800,1101529088,1094713344,1095761920,1096810496,1097859072,1098907648,1096810496,1097859072,1098907648,1099431936,1099956224,1098907648,1099431936,1099956224,1100480512,1101004800,1099956224,1100480512,1101004800,1101529088,1102053376,1101004800,1101529088,1102053376,1102577664,1103101952 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 5 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 -500 2 2 3 0";
/// vv2v line 386.
const V2V_FS270: &str = "a2vmu 4 4 3 1 1 0 i0,2,1,2,0,0,0,2,0,0,0,1,0,1,1,0,0,1,1,2,0,0,0,0,2,1,1,0,0,2,0,0,2,0,0,0,0,0,0,1,1,0,0,0,0,0,0,0 40000000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 c0800000 00000000 40a00000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 1 3f800000 x 00000000 00000000 00000000 3f800000 4 4 3f6a3962 3b0ce0dd 3d8acb80 bd1f725b 3c1abead 3f82deef bd914cf7 bdcf2c1f 3d050f72 3d85f95f 3f82ac3b bf6a67c2 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 4 3 1 40000000 40400000 40000000 1 bf7081b0 bf67b923 3c9cdead bee62f01 bf7378c8 3efc361b 3f5180d0 3ed0ff1b bf7a006a 41500000 c2792092 c1900000 00000000 00000000 00000000 x 1 null 1 2 3 3 3fc00000 3f000000 3f000000 1 bf800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 c0400000 c0000000 3f800000 00000000 00000000 00000000 x 1 null 00000000 00000000 00000000 3f800000 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 2 3 3 3fc00000 3f000000 3f000000 1 bf800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 c0400000 c0000000 3f800000 00000000 00000000 00000000 x 1 null 1 4 3 1 40000000 40400000 40000000 1 bf7081b0 bf67b923 3c9cdead bee62f01 bf7378c8 3efc361b 3f5180d0 3ed0ff1b bf7a006a 41500000 c2792092 c1900000 00000000 00000000 00000000 x 1 null 3fe0000000000000 1 null";

/// vrules19 line 43 (0-based)'s volume encoding (5 x 5 x 5 floats, one frame) around its fill.
const R19_VXN_HEAD: &str = "5 5 5 3 1 0";
const R19_VXN_MID: &str = "3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 5 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0";

/// The result's tokens.
fn res_of(r: &str) -> Vec<String> {
    r.split(" | ")
        .next()
        .unwrap_or("")
        .split(' ')
        .map(str::to_owned)
        .collect()
}

/// FS-269 (`vrules18`): a 6 x 5 x 4 mask filling columns 1 to 3, rows 1 to 2, slices 1 to 2, pad 0:
/// FreeSurfer's `1 1 1  2 1 1`, by default `1 1 1  3 2 2`; noted in both modes.
#[test]
fn fs269_a_box_one_voxel_short() {
    check("FS-269", true, R18_PAD0);
    let (fix, _) = run(R18_PAD0, UpstreamDefects::Fix);
    let (rep, _) = run(R18_PAD0, UpstreamDefects::Reproduce);
    assert_eq!(res_of(&fix), ["1", "1", "1", "3", "2", "2"]);
    assert_eq!(res_of(&rep), ["1", "1", "1", "2", "1", "1"]);
}

/// FS-269 (`vrules18`): with a pad of 1 the box reaches the mask's last voxel: the same in both
/// modes, nothing noted.
#[test]
fn fs269_a_pad_of_one_unchanged() {
    let (fix, ids) = run(R18_PAD1, UpstreamDefects::Fix);
    let (rep, _) = run(R18_PAD1, UpstreamDefects::Reproduce);
    assert_eq!(fix, rep);
    assert!(!ids.iter().any(|i| i == "FS-269"), "{ids:?}");
    assert_eq!(res_of(&fix), ["0", "0", "0", "4", "3", "3"]);
}

/// FS-269 in `REGIONgetBoundingBoxEqOdd` (`vrules18`): FreeSurfer's `1 1 1  2 2 1` returned as it
/// is, by default the box `1 1 1  3 3 2` grown to an odd depth; a box whose decided width passes
/// the height: FreeSurfer's grown box, by default the error and none.
#[test]
fn fs269_the_even_odd_box() {
    let (fix, _) = run(R18_EQODD, UpstreamDefects::Fix);
    let (rep, _) = run(R18_EQODD, UpstreamDefects::Reproduce);
    assert_eq!(res_of(&fix), ["1", "1", "1", "3", "3", "3"]);
    assert_eq!(res_of(&rep), ["1", "1", "1", "2", "2", "1"]);
    let (fix, _) = run(R18_WIDE, UpstreamDefects::Fix);
    let (rep, _) = run(R18_WIDE, UpstreamDefects::Reproduce);
    assert_eq!(res_of(&fix), ["null"]);
    assert!(
        out_of(&fix).contains("dx is bigger than MRI height"),
        "{fix}"
    );
    assert_eq!(res_of(&rep), ["0", "0", "0", "3", "3", "1"]);
}

/// FS-269 in `MRImaskAndUpsample` (`vrules18`): a 2 x 2 x 1 mask, pad 0, factor 2: FreeSurfer's cut
/// of 2 x 2 x 0 voxels, by default 4 x 4 x 2.
#[test]
fn fs269_the_mask_and_upsample_cut() {
    let (fix, _) = run(R18_MASKUP, UpstreamDefects::Fix);
    let (rep, _) = run(R18_MASKUP, UpstreamDefects::Reproduce);
    assert_eq!(res_of(&fix)[..3], ["4", "4", "2"]);
    assert_eq!(res_of(&rep)[..3], ["2", "2", "0"]);
}

/// FS-270 (`vv2v`): `MRIaseg2volMU` with upsampling of a segmentation at neither end of the
/// registration: the concatenation's refusal, then the end in both modes (status 252), noted.
#[test]
fn fs270_a_segmentation_at_neither_end() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(V2V_FS270, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(
            out_of(&r).contains("ERROR: LTAconcat(): LTAs 0 and 1 do not match"),
            "{r}"
        );
        assert!(ids.iter().any(|i| i == "FS-270"), "{ids:?}");
    }
}

/// `MRIaseg2volMU` with upsampling (`vrules17`): the segmentation the array's destination: the
/// modes agree, nothing of FS-270.
#[test]
fn aseg2vol_mu_upsampled_from_the_destination() {
    let (fix, ids) = run(R17_A2VMU, UpstreamDefects::Fix);
    let (rep, _) = run(R17_A2VMU, UpstreamDefects::Reproduce);
    assert_eq!(fix, rep);
    assert!(!ids.iter().any(|i| i == "FS-270"), "{ids:?}");
    assert!(!fix.starts_with("EXIT"), "{fix}");
}

/// FS-482 (`vrules17`): a tensor shift map the sampler refuses: the shift never written is 0 in
/// both modes (every voxel skipped), noted; the modes' volumes agree.
#[test]
fn fs482_a_shift_never_written() {
    let (fix, fids) = run(R17_FS482, UpstreamDefects::Fix);
    let (rep, rids) = run(R17_FS482, UpstreamDefects::Reproduce);
    assert!(fids.iter().any(|i| i == "FS-482"), "{fids:?}");
    assert!(rids.iter().any(|i| i == "FS-482"), "{rids:?}");
    assert_eq!(fix.split(" | ").next(), rep.split(" | ").next());
}

/// FS-483 (`vrules19`): masks of 0.5 everywhere and of `1 0.5 0 NaN` on a 2 x 2 x 1 volume of two
/// frames: FreeSurfer's error, and its sums divided by its count of 1; by default the voxels the
/// sums take (`7.5 9.5 9.5 17.875`, and the sums divided by 3); noted in both modes.
#[test]
fn fs483_the_mask_count() {
    check("FS-483", true, R19_HALF);
    check("FS-483", true, R19_MIXED);
    let (rep, _) = run(R19_HALF, UpstreamDefects::Reproduce);
    assert!(out_of(&rep).contains("ERROR: no voxels in mask"), "{rep}");
    let (fix, _) = run(R19_HALF, UpstreamDefects::Fix);
    let want: Vec<String> = [7.5f32, 9.5, 9.5, 17.875]
        .iter()
        .map(|x| format!("{:08x}", x.to_bits()))
        .collect();
    let got = res_of(&fix);
    assert!(got.ends_with(&want), "{fix}");
}

/// FS-484 (`vrules19`): the covariance of a volume without frames is a 0 x 0 matrix; its PCA ends
/// the program in both modes (status 252), noted.
#[test]
fn fs484_a_pca_without_frames() {
    let (fix, _) = run(R19_COV0, UpstreamDefects::Fix);
    let (rep, _) = run(R19_COV0, UpstreamDefects::Reproduce);
    assert_eq!(fix, rep);
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R19_PCA0, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(ids.iter().any(|i| i == "FS-484"), "{ids:?}");
    }
}

/// FS-323 in §44 (`vrules19`): a point at column -500 with a window of 3: the window's column
/// -501 lies past the index tables: the program ends in both modes (status 252), noted.
#[test]
fn fs323_a_window_past_the_index_tables() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R19_VXN500, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(ids.iter().any(|i| i == "FS-323"), "{ids:?}");
    }
}

/// A `vxn` line on the 5 x 5 x 5 float volume `v = x + 2 y + 3 z` for the points given.
fn vxn_line(points: &[[i32; 5]]) -> String {
    let mut words = Vec::new();
    for z in 0..5 {
        for y in 0..5 {
            for x in 0..5 {
                words.push(((x + 2 * y + 3 * z) as f32).to_bits().to_string());
            }
        }
    }
    let mut s = format!(
        "vxn {R19_VXN_HEAD} i{} {R19_VXN_MID} {}",
        words.join(","),
        points.len()
    );
    for p in points {
        for v in p {
            s.push_str(&format!(" {v}"));
        }
    }
    s
}

/// §44's `MRIvoxelDirection` by its text with the full window (FS-485's default), on `v = x + 2 y
/// + 3 z` of 5 x 5 x 5 voxels (every value, gradient and sum exact in `float`).
fn direction_model(p: [i32; 3], h: i32) -> f32 {
    let v = |x: i32, y: i32, z: i32| f64::from(x + 2 * y + 3 * z);
    let t = |i: i32| i.clamp(0, 4);
    let side = |f: f64, e1: f64, e2: f64| e2 + (2.0 * f + e1);
    let grad = |q: [i32; 3]| {
        let (x, y, z) = (t(q[0]), t(q[1]), t(q[2]));
        let (xm, xp, ym, yp, zm, zp) = (t(x - 1), t(x + 1), t(y - 1), t(y + 1), t(z - 1), t(z + 1));
        let dx = (side(v(xp, y, z), v(xp, ym, z), v(xp, yp, z))
            - side(v(xm, y, z), v(xm, ym, z), v(xm, yp, z)))
            * 0.125;
        let dy = (side(v(xm, yp, z), v(x, yp, z), v(xp, yp, z))
            - side(v(xm, ym, z), v(x, ym, z), v(xp, ym, z)))
            * 0.125;
        let dz = (side(v(xm, y, zp), v(x, y, zp), v(xp, y, zp))
            - side(v(xm, y, zm), v(x, y, zm), v(xp, y, zm)))
            * 0.125;
        [dx, dy, dz]
    };
    let g0 = grad(p);
    let mut dir = 0.0;
    for dz in -h..=h {
        for dy in -h..=h {
            for dx in -h..=h {
                let g = grad([p[0] + dx, p[1] + dy, p[2] + dz]);
                let t1 = f64::from(dx) * g0[0] + f64::from(dy) * g0[1] + f64::from(dz) * g0[2];
                let t2 = g[0] * g0[0] + g[1] * g0[1] + g[2] * g0[2];
                dir += t1 * t2;
            }
        }
    }
    (dir as f32) * f32::from_bits(0x381B_5837)
}

/// FS-485 (§44, not in the probe): windows past the far faces (the corner, a face) and one inside:
/// by default the full window through the index tables, as §44's text computes it; with
/// `reproduce` FreeSurfer's cut window read back as a full cube (another value); noted in both
/// modes only where the window passes a far face.
#[test]
fn fs485_a_window_past_the_far_faces() {
    for (p, w, past) in [
        ([4, 4, 4], 3, true),
        ([2, 4, 1], 5, true),
        ([2, 2, 2], 3, false),
    ] {
        let line = vxn_line(&[[p[0], p[1], p[2], w, 0]]);
        let (fix, fids) = run(&line, UpstreamDefects::Fix);
        let (rep, rids) = run(&line, UpstreamDefects::Reproduce);
        let got = u32::from_str_radix(&res_of(&fix)[4], 16).expect("a float's bits");
        assert_eq!(got, direction_model(p, w / 2).to_bits(), "{line}");
        assert_eq!(fids.iter().any(|i| i == "FS-485"), past, "{fids:?}");
        assert_eq!(rids.iter().any(|i| i == "FS-485"), past, "{rids:?}");
        assert_eq!(res_of(&fix)[4] != res_of(&rep)[4], past, "{fix} {rep}");
    }
}

// ---- part 10a (vrules20): FS-487, FS-488, FS-489, FS-490, FS-12 in HISTOseg, TransformWithMatrix

/// vrules20 line 26's volume and array of no transform, through MvoxelFromTalairach.
const R20_TALMAT_EMPTY: &str = "talmat 2 3 3 2 3 1 0 i0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,1088421888,1090519040,1091567616,1092616192,1093664768,1094713344,1095761920,1096810496,1097859072,1098907648,1099431936 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 1 3f800000 x";
/// vrules20 line 54: MRIvoxelToTalairachEx through a 3 x 3 transform matrix.
const R20_PT0_3X3: &str = "talpt 0 3 3 2 3 1 0 i0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,1088421888,1090519040,1091567616,1092616192,1093664768,1094713344,1095761920,1096810496,1097859072,1098907648,1099431936 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 1 3f800000 x 00000000 00000000 00000000 3f800000 3 3 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 3 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 x 1 null 1 4 4 4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 40a00000 40c00000 40e00000 00000000 00000000 00000000 x 1 null 00000000 00000000 00000000 3f800000 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 4 4 4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 40a00000 40c00000 40e00000 00000000 00000000 00000000 x 1 null 1 3 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 x 1 null 2 3ff0000000000000 4000000000000000 3fe0000000000000 bff0000000000000 0000000000000000 4008000000000000";
/// vrules20 line 62: MRItalairachToVoxelEx through a 3 x 3 transform matrix.
const R20_PT2_3X3: &str = "talpt 2 3 3 2 3 1 0 i0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,1088421888,1090519040,1091567616,1092616192,1093664768,1094713344,1095761920,1096810496,1097859072,1098907648,1099431936 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 1 1 3f800000 x 00000000 00000000 00000000 3f800000 3 3 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 3 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 x 1 null 1 4 4 4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 40a00000 40c00000 40e00000 00000000 00000000 00000000 x 1 null 00000000 00000000 00000000 3f800000 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 4 4 4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 40a00000 40c00000 40e00000 00000000 00000000 00000000 x 1 null 1 3 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 x 1 null 2 3ff0000000000000 4000000000000000 3fe0000000000000 bff0000000000000 0000000000000000 4008000000000000";
/// vrules20 line 80: MRIfromTalairachEx without a target.
const R20_FROMTAL_NEW: &str = "talvol 2 3 3 2 3 1 0 i0,1065353216,1073741824,1077936128,1082130432,1084227584,1086324736,1088421888,1090519040,1091567616,1092616192,1093664768,1094713344,1095761920,1096810496,1097859072,1098907648,1099431936 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new 1 1 3f800000 x 00000000 00000000 00000000 3f800000 4 4 3f800000 00000000 00000000 40000000 00000000 3f800000 00000000 c0400000 00000000 00000000 3f800000 40800000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 3 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 x 1 null 1 4 4 4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 40a00000 40c00000 40e00000 00000000 00000000 00000000 x 1 null 00000000 00000000 00000000 3f800000 4 4 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 4 4 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0 0 1 4 4 4 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 40a00000 40c00000 40e00000 00000000 00000000 00000000 x 1 null 1 3 3 2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 3f800000 c0000000 40400000 00000000 00000000 00000000 x 1 null";
/// vrules20 line 95: MRImatchSegs of 2 and 3 into the segmentation itself.
const R20_MSEGS_SELF: &str = "msegs 3 2 2 1 1 0 i1,2,3,2,2,0,1,1,3,5,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 self 2 2 3 9";
/// vrules20 line 96: the same into a copy.
const R20_MSEGS_NEW: &str = "msegs 3 2 2 1 1 0 i1,2,3,2,2,0,1,1,3,5,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new 2 2 3 9";
/// vrules20 line 97: MRImatchSegs of 0 into the segmentation itself.
const R20_MSEGS_SELF0: &str = "msegs 3 2 2 1 1 0 i1,2,3,2,2,0,1,1,3,5,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 self 1 0 4";
/// vrules20 line 104: HISTOseg of 0 bins.
const R20_HSEG_0: &str = "hseg 3 2 2 1 1 0 i1,2,3,2,2,0,1,1,3,5,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 3 2 2 3 1 0 i0,1069547520,1077936128,1083179008,1086324736,1089470464,1091567616,1093140480,1094713344,1096286208,1097859072,1099169792 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 0000000000000000 c000000000000000 4000000000000000";
/// vrules20 line 108: HISTOseg of a NaN width.
const R20_HSEG_NANW: &str = "hseg 3 2 2 1 1 0 i1,2,3,2,2,0,1,1,3,5,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 3 2 2 3 1 0 i0,1069547520,1077936128,1083179008,1086324736,1089470464,1091567616,1093140480,1094713344,1096286208,1097859072,1099169792 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 0000000000000000 4024000000000000 7ff8000000000000";
/// vrules20 line 110: HISTOseg of NaN values.
const R20_HSEG_NANV: &str = "hseg 3 2 2 1 1 0 i1,2,3,2,2,0,1,1,3,5,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 3 2 2 3 1 0 i1065353216,2143289344,1073741824,2143289344,1082130432,1084227584,1065353216,2143289344,1073741824,2143289344,1082130432,1084227584 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 2 0000000000000000 4024000000000000 4000000000000000";

/// The standard error of a line `run` gave (decoded, the notices taken out).
fn err_of(r: &str) -> String {
    let (_, streams) = r.split_once(" | ").expect("a result line");
    streams
        .split_once(' ')
        .map_or(String::new(), |(_, e)| e.to_owned())
}

/// The line's result values (`res_of`), its notices' IDs and the whole line.
fn r20_res(line: &str, mode: UpstreamDefects) -> (Vec<String>, Vec<String>, String) {
    let (r, ids) = run(line, mode);
    (res_of(&r), ids, r)
}

/// FS-487 (`vrules20`): an array of no transform through `MvoxelFromTalairach`: no matrix in both
/// modes (FreeSurfer reads past the array), `MatrixInverse`'s end (status 252), noted.
#[test]
fn fs487_an_array_of_no_transform() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R20_TALMAT_EMPTY, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(ids.iter().any(|i| i == "FS-487"), "{ids:?}");
    }
}

/// FS-487 (`vrules20`): a 3 x 3 transform matrix through `MRIvoxelToTalairachEx` and
/// `MRItalairachToVoxelEx`: the product's size error, then in both modes each point unset (7)
/// with status 0 and larmorx's error (FreeSurfer crashes), noted.
#[test]
fn fs487_a_point_without_a_matrix() {
    let unset = format!("{:016x}", 7.0f64.to_bits());
    for (line, name) in [
        (R20_PT0_3X3, "MRIvoxelToTalairachEx"),
        (R20_PT2_3X3, "MRItalairachToVoxelEx"),
    ] {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (res, ids, r) = r20_res(line, mode);
            assert_eq!(res.len(), 8, "{r}");
            for k in [0, 4] {
                assert_eq!(res[k], "0", "{r}");
                assert!(res[k + 1..k + 4].iter().all(|v| *v == unset), "{r}");
            }
            let want = format!("error: {name}: no matrix to transform the point with\n");
            assert_eq!(err_of(&r).matches(&want).count(), 2, "{r}");
            assert!(ids.iter().any(|i| i == "FS-487"), "{ids:?}");
        }
    }
}

/// FS-488 (`vrules20`): `MRIfromTalairachEx` without a target: in both modes the check's end
/// first (status 252; FreeSurfer crashes before it), noted.
#[test]
fn fs488_no_target_volume() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R20_FROMTAL_NEW, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(
            err_of(&r).contains("fs_volume_probe: Needs target volume to recover c_(ras)"),
            "{r}"
        );
        assert!(ids.iter().any(|i| i == "FS-488"), "{ids:?}");
    }
}

/// FS-489 (`vrules20`): the segmentation as its own mask: by default each label read before its
/// voxel is cleared (the same mask as into a copy); with `reproduce` every label 0 (a mask of
/// zeros for 2 and 3, of the value everywhere for 0); noted in both modes.
#[test]
fn fs489_the_segmentation_as_its_own_mask() {
    check("FS-489", true, R20_MSEGS_SELF);
    check("FS-489", true, R20_MSEGS_SELF0);
    let voxels = |line: &str, mode| -> String {
        let (res, _, r) = r20_res(line, mode);
        assert!(!r.starts_with("EXIT"), "{r}");
        res.iter()
            .rev()
            .find(|t| t.starts_with('x'))
            .cloned()
            .unwrap_or_default()
    };
    let copy = voxels(R20_MSEGS_NEW, UpstreamDefects::Fix);
    assert_eq!(voxels(R20_MSEGS_SELF, UpstreamDefects::Fix), copy);
    let zeros = format!("x{}", "00000000".repeat(12));
    assert_eq!(voxels(R20_MSEGS_SELF, UpstreamDefects::Reproduce), zeros);
    let fours = format!("x{}", "04000000".repeat(12));
    assert_eq!(voxels(R20_MSEGS_SELF0, UpstreamDefects::Reproduce), fours);
}

/// FS-490 (`vrules20`): `HISTOseg` of 0 bins counts nowhere in both modes (FreeSurfer counts
/// before its array), noted; the modes' results agree.
#[test]
fn fs490_a_histogram_of_no_bins() {
    check("FS-490", false, R20_HSEG_0);
    let (res, _, r) = r20_res(R20_HSEG_0, UpstreamDefects::Fix);
    assert_eq!(res[..2], ["0".to_owned(), "0".to_owned()], "{r}");
}

/// FS-12 in `HISTOseg` (`vrules20`): a NaN width gives `-2147483648` bins with `reproduce` (the
/// allocation's end, status 254) and 0 by default (FS-490 then); NaN values go to bin 0 in both
/// modes.
#[test]
fn fs12_histoseg_a_nan_width_and_nan_values() {
    let (rep, rids) = run(R20_HSEG_NANW, UpstreamDefects::Reproduce);
    assert!(rep.starts_with("EXIT 254 |"), "{rep}");
    assert!(
        err_of(&rep).contains("HISTOalloc(-2147483648): could not allocate histogram"),
        "{rep}"
    );
    assert!(rids.iter().any(|i| i == "FS-12"), "{rids:?}");
    let (fix, fids) = run(R20_HSEG_NANW, UpstreamDefects::Fix);
    assert!(fix.starts_with("0 0 "), "{fix}");
    assert!(
        fids.iter().any(|i| i == "FS-12") && fids.iter().any(|i| i == "FS-490"),
        "{fids:?}"
    );
    check("FS-12", false, R20_HSEG_NANV);
}

/// `TransformWithMatrix` (§46) against its text: of two NaN operands, rows 1 and 2 keep the
/// matrix element's in each product, row 3 the coordinate's.
#[test]
fn transform_with_matrix_nan_operands() {
    use larmorx_fs_core::maths::matrix::{MATRIX_REAL, Matrix};
    use larmorx_fs_core::sse::widen;
    use larmorx_fs_core::volume::talairach::transform_with_matrix;
    // The double coordinate is made by its bits (`widen`), not by `f64::from`: Rust leaves the
    // payload of a NaN converted between float types to the compiler, and an optimised build
    // folds `f64::from(<a NaN constant>)` to the preferred NaN `0x7ff8000000000000`.
    let (a, b) = (
        f32::from_bits(0x7fc0_1234),
        widen(f32::from_bits(0x7fc0_5678)),
    );
    let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
    for k in 1..=4 {
        m.set(k, k, 1.0);
    }
    for r in 1..=3 {
        m.set(r, 1, a);
    }
    let p = transform_with_matrix(&m, b, 2.0, 3.0).expect("a 4 x 4 matrix");
    // the float NaN with payload `q` widened: sign 0, quiet, the payload's 22 bits moved up 29
    let wide = |q: u64| 0x7ff8_0000_0000_0000u64 | (q << 29);
    let got: Vec<u64> = p.iter().map(|v| v.to_bits()).collect();
    assert_eq!(got, [wide(0x1234), wide(0x1234), wide(0x5678)]);
    // A NaN made from numbers (an infinity times 0) is x86-64's default NaN `0xffc00000` on
    // every platform (AArch64's multiplication would give `0x7fc00000`), and of it and a NaN
    // element met in the sum the left partial sum's is kept.
    let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
    m.set(1, 1, f32::INFINITY);
    m.set(2, 2, f32::INFINITY);
    m.set(2, 4, f32::from_bits(0x7fc0_4321));
    m.set(3, 3, f32::INFINITY);
    let p = transform_with_matrix(&m, 0.0, 0.0, 0.0).expect("a 4 x 4 matrix");
    let got: Vec<u64> = p.iter().map(|v| v.to_bits()).collect();
    assert_eq!(got, [0xfff8_0000_0000_0000; 3]);
}

// ---- part 10b (vrules22): FS-491 to FS-497

/// vrules22 line 0: MRIconformedTemplate with KeepDC of a volume without a good RAS (`???`).
const R22_TEMPLATE_NOAXIS: &str = "conform 2 3 2 2 3 1 0 i1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216 3f800000 3f800000 3f800000 0 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 8 3ff0000000000000 1";
/// vrules22 line 2: MRIconformedTemplate with KeepDC of two axes along one direction (`RRS`).
const R22_TEMPLATE_RRS: &str = "conform 2 3 2 2 3 1 0 i1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 8 3ff0000000000000 1";
/// vrules22 line 5: MRIconformedTemplate with KeepDC, width 256 and size 1e10.
const R22_TEMPLATE_HUGE: &str = "conform 2 3 2 2 3 1 0 i1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 256 4202a05f20000000 1";
/// vrules22 line 6: MRIconformedTemplate with KeepDC, width 3 and size 1e10.
const R22_TEMPLATE_HUGE3: &str = "conform 2 3 2 2 3 1 0 i1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216,1065353216 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 3 4202a05f20000000 1";
/// vrules22 line 74: MRIthresholdMask of bytes, the replacement 300.
const R22_TMASK_300: &str = "tmask 4 2 1 0 1 0 i673059850,1346780210 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 2 1 0 1 0 i131328,16777729 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new 3f800000 43960000";
/// vrules22 line 75: MRIthresholdMask of bytes, the replacement -1.
const R22_TMASK_M1: &str = "tmask 4 2 1 0 1 0 i673059850,1346780210 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 2 1 0 1 0 i131328,16777729 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new 3f800000 bf800000";
/// vrules22 line 77: MRIthresholdMask of bytes, the replacement 1e10.
const R22_TMASK_1E10: &str = "tmask 4 2 1 0 1 0 i673059850,1346780210 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 2 1 0 1 0 i131328,16777729 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new 3f800000 501502f9";
/// vrules22 line 83: MRIpercentThresh at the list's end.
const R22_PTHR_END: &str = "pthr 3 2 2 3 1 0 i1084227584,1077936128,1091567616,1065353216,1088421888,1073741824,1090519040,1082130432,1086324736,0,1093664768,1092616192 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 3 2 2 3 1 0 i1065353216,0,0,1065353216,0,0,1065353216,0,0,1065353216,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 0000000000000000";
/// vrules22 line 85: MRIpercentThresh past the voxel count.
const R22_PTHR_PAST: &str = "pthr 3 2 2 3 1 0 i1084227584,1077936128,1091567616,1065353216,1088421888,1073741824,1090519040,1082130432,1086324736,0,1093664768,1092616192 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 3 2 2 3 1 0 i1065353216,0,0,1065353216,0,0,1065353216,0,0,1065353216,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 c069000000000000";
/// vrules22 line 93: MRIpercentThresh of an empty mask.
const R22_PTHR_EMPTY: &str = "pthr 3 2 2 3 1 0 i1084227584,1077936128,1091567616,1065353216,1088421888,1073741824,1090519040,1082130432,1086324736,0,1093664768,1092616192 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 3 2 2 3 1 0 i0,0,0,0,0,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0 4049000000000000";
/// vrules22's hang line 0: MRIerodeSegmentation with a count of 0.
const R22_ERODESEG_0: &str = "erodeseg 4 4 3 1 1 0 i0,1,1,2,0,0,3,3,1,0,2,2,0,1,1,2,0,0,3,3,1,0,2,2,0,1,1,2,0,0,3,3,1,0,2,2,0,1,1,2,0,0,3,3,1,0,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new 0 0";
/// vrules22's hang line 3: MRIdilateSegmentation with a count of 0.
const R22_DILSEG_0: &str = "dilseg 4 4 3 1 1 0 i0,1,1,2,0,0,3,3,1,0,2,2,0,1,1,2,0,0,3,3,1,0,2,2,0,1,1,2,0,0,3,3,1,0,2,2,0,1,1,2,0,0,3,3,1,0,2,2 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 3 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new 0 null 0 3fe0000000000000";
/// vrules22's hang line 5: MRIdilateSegmentation to the end of a complex volume whose first voxel holds the label.
const R22_DILSEG_COMPLEX: &str = "dilseg 4 2 1 7 1 0 i1073741824,0,0,0,0,0,0,0,1073741824,0,0,0,0,0,0,0 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 new -1 null 0 3fe0000000000000";

/// The last token of a line's result (a volume's voxels).
fn r22_voxels(line: &str, mode: UpstreamDefects) -> String {
    let (r, _) = run(line, mode);
    res_of(&r).last().cloned().unwrap_or_default()
}

/// FS-491 (`vrules22`): `MRIconformedTemplate` with `KeepDC` of an orientation without one of
/// the three axes: by default the program ends with the orientation's error, with `reproduce`
/// at FreeSurfer's product of a matrix that has no inverse (status 252 both); noted in both.
#[test]
fn fs491_an_orientation_without_an_axis() {
    for line in [R22_TEMPLATE_NOAXIS, R22_TEMPLATE_RRS] {
        let (fix, fids) = run(line, UpstreamDefects::Fix);
        assert!(fix.starts_with("EXIT 252 |"), "{fix}");
        assert!(
            err_of(&fix)
                .contains("lacks a left/right, inferior/superior or anterior/posterior axis"),
            "{fix}"
        );
        assert!(fids.iter().any(|i| i == "FS-491"), "{fids:?}");
        let (rep, rids) = run(line, UpstreamDefects::Reproduce);
        assert!(rep.starts_with("EXIT 252 |"), "{rep}");
        assert!(
            err_of(&rep).contains("MatrixMultiplyD(): m2 is null"),
            "{rep}"
        );
        assert!(rids.iter().any(|i| i == "FS-491"), "{rids:?}");
    }
}

/// FS-492 (`vrules22`): fields of view beyond the `int` range (size 1e10), clamped by default and
/// -2147483648 with `reproduce`; noted in both modes; the template the same here (the paddings
/// round to 0 either way).
#[test]
fn fs492_fields_of_view_beyond_int() {
    check("FS-492", false, R22_TEMPLATE_HUGE);
    check("FS-492", false, R22_TEMPLATE_HUGE3);
}

/// FS-493 (`vrules22`'s hang lines): counts that never end in FreeSurfer end the program in both
/// modes (status 252), noted.
#[test]
fn fs493_counts_that_never_end() {
    for line in [R22_ERODESEG_0, R22_DILSEG_0] {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (r, ids) = run(line, mode);
            assert!(r.starts_with("EXIT 252 |"), "{r}");
            assert!(err_of(&r).contains("a count of 0 never ends"), "{r}");
            assert!(ids.iter().any(|i| i == "FS-493"), "{ids:?}");
        }
    }
}

/// FS-494 (`vrules22`): the byte replacement of `MRIthresholdMask`: 300, -1 and 1e10 clamped to
/// 255, 0 and 255 by default, FreeSurfer's low bytes 44, 255 and 0 with `reproduce`; noted.
#[test]
fn fs494_replacement_bytes() {
    for (line, fix, rep) in [
        (R22_TMASK_300, "xff141eff323cff50", "x2c141e2c323c2c50"),
        (R22_TMASK_M1, "x00141e00323c0050", "xff141eff323cff50"),
        (R22_TMASK_1E10, "xff141eff323cff50", "x00141e00323c0050"),
    ] {
        check("FS-494", true, line);
        assert_eq!(r22_voxels(line, UpstreamDefects::Fix), fix);
        assert_eq!(r22_voxels(line, UpstreamDefects::Reproduce), rep);
    }
}

/// FS-495 and FS-496 (`vrules22`): `MRIpercentThresh` at the list's end or past the voxel count:
/// the largest value (8) by default, 0 with `reproduce`; an empty mask 0 in both; noted.
#[test]
fn fs495_fs496_percentiles_past_the_list() {
    for (id, line) in [("FS-495", R22_PTHR_END), ("FS-496", R22_PTHR_PAST)] {
        check(id, true, line);
        assert_eq!(r22_voxels(line, UpstreamDefects::Fix), "4020000000000000");
        assert_eq!(
            r22_voxels(line, UpstreamDefects::Reproduce),
            "0000000000000000"
        );
    }
    check("FS-495", false, R22_PTHR_EMPTY);
}

/// FS-497 (`vrules22`'s hang line 5): with `reproduce` (FreeSurfer's copy of a complex row, FS-161)
/// the dilation to the end changes voxels its working copy never sees: the program ends (status
/// 252), noted; by default the whole rows are copied and the dilation ends.
#[test]
fn fs497_a_dilation_whose_changes_never_reach_its_copy() {
    let (rep, rids) = run(R22_DILSEG_COMPLEX, UpstreamDefects::Reproduce);
    assert!(rep.starts_with("EXIT 252 |"), "{rep}");
    assert!(
        err_of(&rep).contains("voxels that the next step does not see"),
        "{rep}"
    );
    assert!(rids.iter().any(|i| i == "FS-497"), "{rids:?}");
    let (fix, _) = run(R22_DILSEG_COMPLEX, UpstreamDefects::Fix);
    assert!(!fix.starts_with("EXIT"), "{fix}");
}

// ---- part 10c (vrules23): FS-498 to FS-502, FS-965

/// vrules23 line 60: MRIhistogram of a constant volume of 300 (one bin of size 0 / 0).
const R23_HIST_300: &str = "hist 2 2 1 4 1 0 i19661100,19661100 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0";
/// vrules23 line 80: MRIgetEqualizeHisto from -1 to 4, not normalised.
const R23_GEQ_LOW_M1: &str = "geq 4 3 1 0 1 0 i33620224,84083459,151587079 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 null -1 4 0";
/// vrules23 line 86: MRIgetEqualizeHisto into a given result of 4 bins for 10.
const R23_GEQ_SMALL: &str = "geq 4 3 1 0 1 0 i33620224,84083459,151587079 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 4 3f800000 00000000 40400000 3f800000 3f800000 3f800000 3f800000 3f800000 3f800000 3f800000 3f800000 0 9 1";
/// vrules23 line 68: MRIgetEqualizeHisto from 0 to 9, not normalised.
const R23_GEQ_0_9: &str = "geq 4 3 1 0 1 0 i33620224,84083459,151587079 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 null 0 9 0";
/// vrules23 line 122: MRIhistoSegment of ints.
const R23_SEGM_INTS: &str = "hsegm 4 1 1 1 1 0 i70,128,110,128 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 4 1 1 0 1 0 i2155905152 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 85 130 95 3 40000000";
/// vrules23 line 123: MRIhistoSegment with an infinity in a later window.
const R23_SEGM_INF: &str = "hsegm 8 1 1 3 1 0 i1117782016,1121714176,1117782016,1121714176,2139095040,1117782016,1121714176,1117782016 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 8 1 1 0 1 0 i128,128 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 85 130 95 3 40000000";
/// vrules23 line 113: MRIhistoSegment of halves (a first window of one value).
const R23_SEGM_HALVES: &str = "hsegm 6 6 2 0 1 0 i1850757200,1347448430,1852730960,1850757200,1347448430,1852730960,1850757200,1347448430,1852730960,1850757200,1347448430,1852730960,1850757200,1347448430,1852730960,1850757200,1347448430,1852730960 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 6 6 2 0 1 0 i2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 85 130 100 5 40000000";
/// vrules23's hang line 0: MRIhistoSegment with a gray-matter limit of -2.
const R23_SEGM_LIMIT_M2: &str = "hsegm 6 6 2 0 1 0 i1349406830,1850757230,1850764880,1349406830,1850757230,1850764880,1349406830,1850757230,1850764880,1850764880,1349414480,1349406830,1850764880,1349414480,1349406830,1850764880,1349414480,1349406830 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 6 6 2 0 1 0 i2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152,2155905152 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 2 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 85 130 -2 5 40000000";
/// a hand line: MRIhistogram of the floats 0 and 1e8 (a range of 100,000,001 bins).
const R23_HIST_1E8: &str = "hist 2 1 1 3 1 0 i0,1287568416 3f800000 3f800000 3f800000 1 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 00000000 00000000 1 1 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000 0000000000000000 0 00000000 00000000 0 1 null null 0 0";

/// FS-498 (`vrules23` line 60): a constant volume of 300 in one bin (the size 0 / 0): with
/// `reproduce` FreeSurfer's bin -2147483648, outside the histogram, is not counted (FreeSurfer
/// crashes), noted; by default FS-12 gives the NaN quotient bin 0, every voxel counted (4).
#[test]
fn fs498_a_bin_outside_the_histogram() {
    let (rep, rids) = run(R23_HIST_300, UpstreamDefects::Reproduce);
    assert!(rids.iter().any(|i| i == "FS-498"), "{rids:?}");
    assert_eq!(res_of(&rep).last().map(String::as_str), Some("00000000"), "{rep}");
    let (fix, fids) = run(R23_HIST_300, UpstreamDefects::Fix);
    assert!(fids.iter().any(|i| i == "FS-12"), "{fids:?}");
    assert_eq!(res_of(&fix).last().map(String::as_str), Some("40800000"), "{fix}");
    // the region histogram of bytes: a first window of one value (MRIhistoSegment), noted with
    // `reproduce`
    let (_, ids) = run(R23_SEGM_HALVES, UpstreamDefects::Reproduce);
    assert!(ids.iter().any(|i| i == "FS-498"), "{ids:?}");
}

/// FS-499 (`vrules23`): the equalisation's bins outside a histogram's allocation (a low limit of
/// -1, a given result of 4 bins for 10): read as 0, writes dropped, in both modes, noted; the
/// counts the same in both modes (FS-500 changes the values).
#[test]
fn fs499_bins_outside_the_allocation() {
    check("FS-499", true, R23_GEQ_LOW_M1);
    check("FS-499", true, R23_GEQ_SMALL);
    let (fix, _) = run(R23_GEQ_SMALL, UpstreamDefects::Fix);
    let (rep, _) = run(R23_GEQ_SMALL, UpstreamDefects::Reproduce);
    let (f, r) = (res_of(&fix), res_of(&rep));
    assert_eq!(f[f.len() - 4..], r[r.len() - 4..]);
    assert_eq!(f[f.len() - 8..f.len() - 4], ["00000000", "3f800000", "40000000", "40400000"]);
    assert_eq!(r[r.len() - 8..r.len() - 4], ["3f800000"; 4]);
}

/// FS-500 (`vrules23` line 68): the cumulative histogram's values from the low to the high limit:
/// their numbers by default, unset (0 in a new result) with `reproduce`; noted in both.
#[test]
fn fs500_the_cumulative_values() {
    check("FS-500", true, R23_GEQ_0_9);
}

/// FS-501 (`vrules23` lines 122 and 123): no region histogram to smooth (ints) and a reused
/// histogram of a negative count (an infinity in a later window) end the program in both modes
/// (statuses 253 and 252), noted.
#[test]
fn fs501_segmentations_freesurfer_crashes_on() {
    for (line, st, msg) in [
        (R23_SEGM_INTS, "EXIT 253 |", "no region histogram to smooth"),
        (R23_SEGM_INF, "EXIT 252 |", "a histogram of -2147483648 bins cannot be cleared"),
    ] {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (r, ids) = run(line, mode);
            assert!(r.starts_with(st), "{r}");
            assert!(err_of(&r).contains(msg), "{r}");
            assert!(ids.iter().any(|i| i == "FS-501"), "{ids:?}");
        }
    }
}

/// FS-502 (`vrules23`'s hang line 0): a gray-matter limit of -2 brings the peak search back to a
/// peak it found: the program ends in both modes (status 252), noted.
#[test]
fn fs502_a_peak_search_that_never_ends() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R23_SEGM_LIMIT_M2, mode);
        assert!(r.starts_with("EXIT 252 |"), "{r}");
        assert!(
            err_of(&r).contains("the gray-matter peak 80 stays above -2 (FreeSurfer never ends here)"),
            "{r}"
        );
        assert!(ids.iter().any(|i| i == "FS-502"), "{ids:?}");
    }
}

/// FS-965 (a hand line, not probed: FreeSurfer allocates gigabytes there): a histogram of
/// 100,000,001 bins taken from the range 0 to 1e8 ends the program in both modes (status 254),
/// noted.
#[test]
fn fs965_a_range_of_too_many_bins() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, ids) = run(R23_HIST_1E8, mode);
        assert!(r.starts_with("EXIT 254 |"), "{r}");
        assert!(
            err_of(&r).contains(
                "HISTOalloc(100000001): a histogram of 100000001 bins (the values' range, 0e0 to 1e8) is refused (FreeSurfer allocates 800000008 bytes here)"
            ),
            "{r}"
        );
        assert!(ids.iter().any(|i| i == "FS-965"), "{ids:?}");
    }
}
