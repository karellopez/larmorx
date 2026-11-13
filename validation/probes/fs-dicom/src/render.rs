// SPDX-License-Identifier: Apache-2.0
//! The dump format of `shim.c` and `port.rs` rendered as text lines, for the differences shown.

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.i)?;
        self.i += 1;
        Some(v)
    }
    fn u32(&mut self) -> Option<u32> {
        let s = self.b.get(self.i..self.i + 4)?;
        self.i += 4;
        Some(u32::from_le_bytes(s.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        let s = self.b.get(self.i..self.i + 8)?;
        self.i += 8;
        Some(u64::from_le_bytes(s.try_into().ok()?))
    }
    fn bytes(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.i..self.i + n)?;
        self.i += n;
        Some(s)
    }
    fn s(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        Some(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
}

fn object(p: &mut P, depth: usize, out: &mut Vec<String>) -> Option<()> {
    let pad = "  ".repeat(depth);
    if p.u8()? != b'J' {
        return None;
    }
    if p.u8()? == b'0' {
        out.push(format!("{pad}object NULL"));
        return Some(());
    }
    let key = p.s()?;
    let (ty, am, del, glf) = (p.u32()?, p.u32()?, p.u32()?, p.u32()?);
    let (size, off, ps, po) = (p.u64()?, p.u64()?, p.u64()?, p.u64()?);
    let (bits, pr, fd) = (p.u32()?, p.u32()?, p.u32()?);
    let name = p.s()?;
    let pf = p.u32()?;
    let pre = p.bytes(128)?.iter().map(|&b| b as u64).sum::<u64>();
    let (dopt, mhl, lvr) = (p.u64()?, p.u64()?, p.u32()?);
    let wf = p.s()?;
    out.push(format!(
        "{pad}object key={key:?} type={ty} access={am} delete={del} grouplength={glf} size={size} offset={off} pixelsize={ps} pixeloffset={po} bits={bits} pixelrep={pr} fd={fd} name={} preamble={pf}/{pre} dataoptions={dopt:#x} meta={mhl:#x} longvr={lvr} waveformvr={wf:?}",
        name.len()
    ));
    let ng = p.u32()?;
    for _ in 0..ng {
        if p.u8()? != b'G' {
            return None;
        }
        let (g, bl, lv, ne) = (p.u32()?, p.u64()?, p.u32()?, p.u32()?);
        out.push(format!("{pad} group {g:04x} base={bl} longvr={lv} elements={ne}"));
        for _ in 0..ne {
            if p.u8()? != b'E' {
                return None;
            }
            let (tag, rep, vm, len) = (p.u32()?, p.u32()?, p.u64()?, p.u32()?);
            let desc = p.s()?;
            let (bo, doff, coff, al, ol, pl, ff) =
                (p.u32()?, p.u64()?, p.u64()?, p.u64()?, p.u64()?, p.u64()?, p.u32()?);
            let mut line = format!(
                "{pad}  ({:04x},{:04x}) vr={rep} vm={vm} len={len:#x} {desc:?} order={bo} dataoffset={doff} cur={coff} alloc={al} orig={ol} padded={pl} frag={ff}",
                tag >> 16,
                tag & 0xffff
            );
            match p.u8()? {
                b'Q' => {
                    let n = p.u32()?;
                    out.push(format!("{line} sequence of {n}"));
                    if n != u32::MAX {
                        for _ in 0..n {
                            object(p, depth + 2, out)?;
                        }
                    }
                    continue;
                }
                b'Z' => line.push_str(" on disk"),
                b'U' => line.push_str(" undefined"),
                b'D' => {
                    let n = p.u32()? as usize;
                    let d = p.bytes(n)?;
                    let h: String = d.iter().take(24).map(|b| format!("{b:02x}")).collect();
                    line.push_str(&format!(" data[{n}]={h}"));
                }
                _ => return None,
            }
            out.push(line);
        }
    }
    Some(())
}

pub fn render(b: &[u8]) -> Vec<String> {
    let mut p = P { b, i: 0 };
    let mut out = Vec::new();
    let ok = (|| -> Option<()> {
        while p.i < p.b.len() {
            match p.u8()? {
                b'O' => {
                    let c = p.u64()?;
                    out.push(format!("open -> {c:#x}"));
                    object(&mut p, 1, &mut out)?;
                }
                b'X' => {
                    let c = p.u64()?;
                    let h = p.u8()?;
                    out.push(format!("close -> {c:#x} handle {}", h as char));
                }
                b'S' => {
                    let n = p.u32()?;
                    out.push(format!("stack {n}"));
                    for _ in 0..n {
                        let c = p.u64()?;
                        let t = p.s()?;
                        out.push(format!("  {c:8x} {t}"));
                    }
                }
                b'P' => {
                    let c = p.u64()?;
                    out.push(format!("pop -> {c:#x}"));
                }
                b'D' => out.push("dump".into()),
                b'T' => {
                    let (c, code) = (p.u64()?, p.u64()?);
                    let t = p.s()?;
                    out.push(format!("top -> {c:#x} {code:#x} {t:?}"));
                }
                x => {
                    out.push(format!("?? record {x}"));
                    return None;
                }
            }
        }
        Some(())
    })();
    if ok.is_none() {
        out.push(format!("(dump cut at byte {} of {})", p.i, b.len()));
    }
    out
}
