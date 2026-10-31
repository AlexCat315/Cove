use cove_imaging::ImageDecoder;

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 { c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 }; }
        *slot = c;
    }
    let mut crc = 0xffff_ffffu32;
    for &b in data { crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8); }
    crc ^ 0xffff_ffff
}
fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}
fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut raw = Vec::new();
    for y in 0..height as usize {
        raw.push(0);
        let start = y * width as usize * 4;
        raw.extend_from_slice(&rgba[start..start + width as usize * 4]);
    }
    let mut zlib = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        zlib.push(if last { 1 } else { 0 });
        zlib.extend_from_slice(&(block.len() as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    let mut adler = (1u32, 0u32);
    for &b in &raw { adler.0 = (adler.0 + b as u32) % 65521; adler.1 = (adler.1 + adler.0) % 65521; }
    zlib.extend_from_slice(&((adler.1 << 16) | adler.0).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    out.extend(chunk(b"IHDR", &ihdr));
    out.extend(chunk(b"IDAT", &zlib));
    out.extend(chunk(b"IEND", &[]));
    out
}

#[test]
fn probe() {
    let w = 4u32; let h = 3u32;
    let mut src = vec![0u8; (w*h*4) as usize];
    let mut enc = src.clone();
    for y in 0..h as usize { for x in 0..w as usize {
        let i = (y*w as usize + x)*4;
        let px: [u8;4] = match x { 0 => [255,0,0,255], 1 => [0,255,0,255], 2 => [0,0,255,255], _ => [255,255,0,128] };
        src[i..i+4].copy_from_slice(&px);
        let a = px[3] as u32;
        enc[i..i+4].copy_from_slice(&[((px[0] as u32*a)/255) as u8, ((px[1] as u32*a)/255) as u8, ((px[2] as u32*a)/255) as u8, a as u8]);
    }}
    let data = png(w,h,&enc);
    let dir = std::env::temp_dir().join(format!("cove-png-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rt.png");
    std::fs::write(&path, &data).unwrap();
    match ImageDecoder::decode(&path, None) {
        Ok(img) => println!("decoded {}x{} px={:?} yellow={:?}", img.width(), img.height(), &img.pixels()[0..4], &img.pixels()[12..16]),
        Err(e) => println!("ERR {e}"),
    }
    std::fs::remove_dir_all(&dir).ok();
}
