// SPDX-License-Identifier: Apache-2.0
// Generated: the record of validation/probes/x87-extended, copied verbatim (rustfmt aside):
// `x87-probe <libx87probe.so> 24 --record <file>` on the reference machine (x86-64, AMD Zen 4,
// GCC 8.4.0's build of x87_ops.c and patterns.cpp), 4,748,983,348 comparisons, 0 differences.
// Recorded by validation/probes/x87-extended (x87-probe --record) from the machine's x87.
// The hardware's results of the first 1024 cases of each (operation, family).
pub(super) const OP_FAMILY_HASHES: [[u64; 17]; 14] = [
    [
        0x7cbed94f203c23d9,
        0x8b8ffb2cbd5f8d8b,
        0x7bf520ba4f4bf67d,
        0xef9df2a1fada849b,
        0x166801ac81d6c6fe,
        0x57181d81f0ded895,
        0xa1358b2a8457e313,
        0xd182434d3f533327,
        0xcedd24005719f438,
        0x23174cd47b12f01f,
        0xac790d010f7fac77,
        0x46b14e0a42d9aaa4,
        0xaa883b49c8368e36,
        0x0be516b74c08bce8,
        0x5e701e68e4132123,
        0x6d3abd7925751109,
        0x8603c377763cfda0,
    ], // Add
    [
        0x170504fce770a805,
        0x311da86c0017ec9e,
        0x0ff273c3d45b8687,
        0x65938f5f206f1091,
        0x0b676a9538a48a49,
        0x1a2735df67d57bb8,
        0x3d74588fb4fb21a8,
        0xcb4f89bbf6c34e83,
        0x5045111756820bcf,
        0x3ecd2ea49b7f00de,
        0x3afef5130be53e46,
        0x2b81ec30710c5af8,
        0xde1cd27a2b785982,
        0xc5f4c05af26c74c6,
        0xa830bcd55c260f81,
        0xdd8b9a2881b53e5a,
        0x669d4f894ad6c3da,
    ], // Sub
    [
        0xfb1b0532af06dec1,
        0xbffe06e5952c2f81,
        0x4a62b4850a29cedb,
        0x11a341a6dd1b2e7a,
        0x0785d88251ae3378,
        0xbee43b69b20a85ba,
        0x9695fa2fa98706a2,
        0x8dac24a474fa4c9e,
        0x8d06c26c1df8507b,
        0x5900429dfcf3bc25,
        0xbc546ca2806f52a0,
        0x4c70831ac4c55e58,
        0xa1faec71151e5dfb,
        0x4326a62ce50e59b9,
        0x9144af0abe0cba11,
        0x35a05cf463ee9f6a,
        0xa28f42c6fdc959fc,
    ], // Mul
    [
        0x88f30ffa975145fe,
        0xd596af49b8acc541,
        0xd5e016f55e797467,
        0x530fda7565c3039b,
        0x1e1e4fffaaa6d57c,
        0xcb8f82bcbbac0cc1,
        0x4dc940954325d157,
        0xec724c460f633268,
        0xc6ad384d989a3367,
        0x622f03abf553f9cf,
        0xedbc02c8ae21dca4,
        0xc31ba7f4d2118ca3,
        0x1b3df31ff22faf36,
        0xd5680b5350583eb0,
        0xaa21c67731408f8c,
        0x50f1486e05d1d6c0,
        0x8092f48496b1ca53,
    ], // Div
    [
        0xe458d91be38965fe,
        0x86815aefde579b0a,
        0xf6d6e2588689ff68,
        0x1ce9716a7dfe04fb,
        0x0a85e0c05e6b17bc,
        0x2542263fad9083dd,
        0x985fa52928202ff6,
        0xd488ca8b8a8417ad,
        0x89deb8521438acb9,
        0xd8fa32f5eabebcb8,
        0xab35b234ea377243,
        0xd408d68b94101a58,
        0x150cd43b0044cd3e,
        0xa69f6d378000f2fa,
        0xd6662603af924e1e,
        0x880b36ec38bbd883,
        0xaf2a4e08d504d820,
    ], // Sqrt
    [
        0x59d679e4bd2bfda5,
        0xaa808a5ec21d7856,
        0xaec54571af50f025,
        0x59d0c2e1f662cbd6,
        0x35f3607319da21e5,
        0x8037961f595947a5,
        0x57613316c02bbaa5,
        0x4791e79946b2ade5,
        0x92cab23aecd4fd16,
        0x56aa83483dd4f765,
        0xb38d2721c1f60125,
        0xb4ffd58726dfef65,
        0x864e18bbca1314bb,
        0x7a03c3f9005e6c66,
        0x05770e3d8a92cc16,
        0x6272f554c05148d6,
        0xf5be80ad222d3e48,
    ], // Cmp
    [
        0x8c3069762b90f1d8,
        0x9c5691e9316c2403,
        0xb2a80c03b03393d0,
        0x711f94d5d4bbd3c9,
        0x97aa47f50aa3cbc3,
        0x47216c6072b69aa5,
        0x5e797bbde73237a5,
        0x1ae69abeda5f8725,
        0x494f1fb64a5cdaa5,
        0x70fd1eaaa7977f25,
        0x0c9bff4988f5b866,
        0xe7cbb458d7550c5f,
        0xd4a8123278101b20,
        0xe02753d1a2c45457,
        0x839a2b36f60fbffd,
        0x97caa5ef8ab7e76c,
        0x6293f6f20e0f25dc,
    ], // ToF64
    [
        0x7721232bdf419f9a,
        0x4f18751b33b7e8f0,
        0x0350213d87ad9609,
        0xf7d3d50c87483aed,
        0x1afd6285630f2dd6,
        0xb8c743d5afbb63a5,
        0xb7f1344aee771b25,
        0x5da16d0462808ba5,
        0x8340d0bf6d723025,
        0x19af3fcc4b02a825,
        0x8a6b0e944189fda6,
        0xc077eb0ea2efc0be,
        0x0929a07a96c27a9b,
        0x0501b4430afbc7c5,
        0x96640c941140c733,
        0xc05a407607143384,
        0x2d15436d5ad0c450,
    ], // ToF32
    [
        0x45625334a65c7003,
        0x804964cf065c2bbe,
        0xb31ce3b3e5e2813a,
        0xb1cbe9c2cbb510bd,
        0x7a755cba019305e9,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x8862a19f53072325,
        0x8862a19f53072325,
        0x9c1bda7f8c872325,
        0x511f547915d76c90,
        0xe25d482e0204f802,
        0xeebbfdb9259d8d02,
        0x117e7844cd418797,
        0xd7f6111943e763ab,
        0x8be54d2cfe3c1089,
        0xffbc9bfbcdaf72a7,
    ], // ToI64
    [
        0x0ef54fa5783a1725,
        0x4524d1e8c43a1dd1,
        0xeba08fd3b6840ba5,
        0x0862d9f26e5d9885,
        0x28ac64ef54a7e7ea,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x84b111c4fb072325,
        0x84b111c4fb072325,
        0x9c1bda7f8c872325,
        0xfdd02250fd66cad4,
        0x13363f75862dc7f1,
        0xe09ef4b93510d62e,
        0x53298fd093a9a37b,
        0xd89d27c0c501b4d1,
        0x740bc4cfb34d9485,
        0x2cdb65e50e93292d,
    ], // ToI32
    [
        0xe85713d72d2b2325,
        0x7a6a8c159b7dd8b2,
        0x372a12a6c397dca5,
        0xf47ec62e3310c386,
        0xd8a63f7a7dfb7da5,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0xca944b59dc872325,
        0xca944b59dc872325,
        0x9c1bda7f8c872325,
        0xd0c4f739a6cc083a,
        0x7866980fa2ef4cc9,
        0xbc6ad144d23e9960,
        0xdedf469661c78b3f,
        0xfa7849dd6968d679,
        0x8d018bea4f1b0fcd,
        0x4aec79b908121316,
    ], // ToI16
    [
        0xf28a4c991a6ca7c9,
        0x69f7d2d1de0f6bdf,
        0xd6078c19f4338d9f,
        0x6ab98db9d57c20f7,
        0xf6bd325c40f3f9e2,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0xc98bff42f58a6125,
        0x7d7d0a8616d13da5,
        0x9c1bda7f8c872325,
        0x465b5d2290cf026c,
        0x655b88cb4875b5ac,
        0xba545de4262a51f6,
        0xe85fc07546b7129c,
        0xaad9cea26379f24e,
        0x9a4bc73b6e1f429e,
        0xf6b29b2fddc8e859,
    ], // ToU64
    [
        0xecb7e149c5347231,
        0x8c447ccca2463131,
        0xd037a0fc0151c6ff,
        0x89339256671fccee,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x912c5d3701767ae6,
        0x93f4f5ce0be8f23f,
        0xb6eb4efe2562b01a,
        0x73685181b9767292,
        0x8e12f320fca6f4cc,
        0x095c573fcc288610,
        0x93da0c623dcf82de,
    ], // ToU32
    [
        0x879aea2d40a78db0,
        0xa0d8d632eb498ba5,
        0xebd2adb2692367b7,
        0x728e7ebc5fc13588,
        0x607e964908e124e1,
        0x9c1bda7f8c872325,
        0x9c1bda7f8c872325,
        0x8862a19f53072325,
        0x8862a19f53072325,
        0x9c1bda7f8c872325,
        0x5df3681f45116c45,
        0xe8b7c89a2dc2b484,
        0xc911a7b1ba22c66b,
        0xe4d7bffff3b03295,
        0x689d003a4ee58042,
        0x80831a42ddfb7ae8,
        0x16b049de5f05a821,
    ], // RoundToI64
];
// Every pair of cases::specials(), in order, per operation.
pub(super) const SPECIAL_HASHES: [u64; 14] = [
    0xe814bb0581428b95, // Add
    0x32944c518c3cc0f5, // Sub
    0x9cea69c43c6b6c9d, // Mul
    0x67348461d9ad524d, // Div
    0x56dd49882be23581, // Sqrt
    0xa5eb700b15ffaf65, // Cmp
    0xddfbff1944c4d43d, // ToF64
    0x3bbc15c001c96625, // ToF32
    0x63df08f5e2e27b75, // ToI64
    0x8d0c8cabb053d82d, // ToI32
    0x50a09421ebfd592d, // ToI16
    0x66eee912592f3b75, // ToU64
    0xbf1f5943c5bddb2d, // ToU32
    0xb1e45047f1a483e5, // RoundToI64
];
// The first 1024 inputs of each conversion into the extended format.
pub(super) const CONV_HASHES: [u64; 6] = [
    0x254da1209ab7ed48, // F64
    0x7105f952d6829373, // F32
    0x8b9d3f022838fa41, // I64
    0x4903accb57f06fd4, // I32
    0xd6052cc6cd9cdc2c, // U64
    0xdb932478b5d4fcf6, // U32
];
// The outputs of the first 256 cases of each pattern.
pub(super) const PATTERN_HASHES: [u64; 6] = [
    0xe3a40a95e119626e, // Dot
    0x791181ef2502b7d7, // Stats
    0x70c287c5b19e1ebb, // Cross
    0x84e81adefa0980f0, // Unit
    0xc959a699adf64407, // Smooth
    0x88b20546ad8520f4, // EdgeCost
];
