use std::fs;
use std::path::Path;

// === BINARY PRIMITIVES ===

fn write_u32(buf: &mut Vec<u8>, val: u32) {
    buf.extend_from_slice(&val.to_le_bytes());
}

fn write_f32(buf: &mut Vec<u8>, val: f32) {
    buf.extend_from_slice(&val.to_le_bytes());
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

fn read_u32(data: &[u8], pos: &mut usize) -> u32 {
    let val = u32::from_le_bytes([data[*pos], data[*pos+1], data[*pos+2], data[*pos+3]]);
    *pos += 4;
    val
}

fn read_f32(data: &[u8], pos: &mut usize) -> f32 {
    let val = f32::from_le_bytes([data[*pos], data[*pos+1], data[*pos+2], data[*pos+3]]);
    *pos += 4;
    val
}

fn read_string(data: &[u8], pos: &mut usize) -> String {
    let len = read_u32(data, pos) as usize;
    if *pos + len > data.len() {
        return format!("<overflow: len={} pos={} total={}>", len, *pos, data.len());
    }
    let s = String::from_utf8_lossy(&data[*pos..*pos+len]).to_string();
    *pos += len;
    s
}

// === KENSHI MOD FORMAT ===
// Decoded from RE analysis of gamedata.base, Godsend EXP 5x, blot smithy fix
//
// itemType enum (from KenshiLib/Include/kenshi/Enums.h):
//   0=BUILDING, 1=CHARACTER, 2=WEAPON, 3=ARMOUR, 4=ITEM,
//   27=CONSTANTS, 70=NEW_GAME_STARTOFF, etc.
//
// Format:
//   HEADER: u32(16) + u32(1) + string(name) + string(desc) + string(dep)
//   DATA:   u32(ref_count=0) + u32(mod_id) + u32(item_count)
//   ITEMS:  [pre(3xu32) + name + stringID + flags + meta + floats + post(7xu32)] × N

#[derive(Clone)]
struct ModItem {
    name: String,
    string_id: String,
    item_type: u32,  // itemType enum value
    floats: Vec<(String, f32)>,
    ints: Vec<(String, i32)>,
}

fn forge_mod(
    output_path: &str,
    mod_name: &str,
    description: &str,
    items: &[ModItem],
) {
    let mut buf: Vec<u8> = Vec::new();

    // === FILE HEADER ===
    write_u32(&mut buf, 16);  // format version (always 16)
    write_u32(&mut buf, 1);   // data version (1 for gameplay mods)
    write_string(&mut buf, mod_name);
    write_string(&mut buf, description);
    write_string(&mut buf, "gamedata.base");

    // === DATA SECTION HEADER ===
    write_u32(&mut buf, 0);           // reference count (0 for simple mods)
    write_u32(&mut buf, 0x00EA1A00);  // mod unique ID (GAIA in hex-speak)
    write_u32(&mut buf, items.len() as u32);

    // === ITEMS ===
    for item in items {
        // PRE-ITEM: 3 uint32s [0, itemType, 0]
        write_u32(&mut buf, 0);
        write_u32(&mut buf, item.item_type);
        write_u32(&mut buf, 0);

        // Item name and string ID
        write_string(&mut buf, &item.name);
        write_string(&mut buf, &item.string_id);

        // Type flags: 0x80000001 = "modified from base"
        write_u32(&mut buf, 0x80000001);

        // 8 data type sections in order:
        // bool, float, int, string, file, vec3, quat, refs

        // 1. BOOL section (count=0 for our mods)
        write_u32(&mut buf, 0);

        // 2. FLOAT section
        write_u32(&mut buf, item.floats.len() as u32);
        for (key, val) in &item.floats {
            write_string(&mut buf, key);
            write_f32(&mut buf, *val);
        }

        // 3. INT section
        write_u32(&mut buf, item.ints.len() as u32);
        for (key, val) in &item.ints {
            write_string(&mut buf, key);
            write_u32(&mut buf, *val as u32);
        }

        // 4-9. Remaining sections: string, file, vec3, quat, refs, instances
        for _ in 0..6 {
            write_u32(&mut buf, 0);
        }
    }

    fs::write(output_path, &buf).expect("Failed to write mod file");
    println!("  FORGED: {} ({} bytes, {} items)", output_path, buf.len(), items.len());
}

fn forge_info_xml(output_path: &str, mod_name: &str, tags: &[&str]) {
    let tag_xml: String = tags.iter()
        .map(|t| format!("    <string>{}</string>", t))
        .collect::<Vec<_>>()
        .join("\n");

    let xml = format!(r#"<?xml version="1.0"?>
<ModData xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <id>0</id>
  <mod>{mod_name}</mod>
  <title>{mod_name}</title>
  <tags>
{tags}
  </tags>
  <visibility>0</visibility>
  <lastUpdate xsi:nil="true" />
</ModData>"#, mod_name = mod_name, tags = tag_xml);

    fs::write(output_path, xml).expect("Failed to write info file");
    println!("  INFO:   {}", output_path);
}

// === DUMP MODE — analyze existing .mod files ===
fn dump_mod(path: &str) {
    let data = fs::read(path).expect("Failed to read mod file");
    let mut pos = 0;

    println!("=== MOD DUMP: {} ({} bytes) ===", path, data.len());

    let format_ver = read_u32(&data, &mut pos);
    let header_ver = read_u32(&data, &mut pos);
    println!("Format: {}, Version: {}", format_ver, header_ver);

    let name = read_string(&data, &mut pos);
    println!("Name: \"{}\"", name);

    let desc = read_string(&data, &mut pos);
    let desc_preview = if desc.len() > 80 { &desc[..80] } else { &desc };
    println!("Description: \"{}...\" ({} chars)", desc_preview, desc.len());

    let dep = read_string(&data, &mut pos);
    println!("Dependency: \"{}\"", dep);

    if pos + 12 > data.len() {
        println!("(no data section — file/resource override mod)");
        return;
    }

    let ref_count = read_u32(&data, &mut pos);
    let mod_id = read_u32(&data, &mut pos);
    let item_count = read_u32(&data, &mut pos);
    println!("Refs: {}, ModID: 0x{:X}, Items: {}", ref_count, mod_id, item_count);

    for i in 0..item_count {
        if pos + 12 > data.len() { break; }

        // PRE-ITEM: 3 uint32s
        let pre0 = read_u32(&data, &mut pos);
        let item_type = read_u32(&data, &mut pos);
        let pre2 = read_u32(&data, &mut pos);

        let type_name = match item_type {
            0 => "BUILDING", 1 => "CHARACTER", 2 => "WEAPON", 3 => "ARMOUR",
            4 => "ITEM", 7 => "RACE", 10 => "FACTION", 13 => "TOWN",
            20 => "TECHTREE", 21 => "RESEARCH", 25 => "STATS",
            27 => "CONSTANTS", 57 => "WEAPON_MANUFACTURER",
            70 => "NEW_GAME_STARTOFF", _ => "?",
        };

        println!("\n  Item {} [{}={}] pre=[{},{},{}]:", i, type_name, item_type, pre0, item_type, pre2);

        let item_name = read_string(&data, &mut pos);
        let string_id = read_string(&data, &mut pos);
        println!("    Name: \"{}\"", item_name);
        println!("    SID:  \"{}\"", string_id);

        let type_flags = read_u32(&data, &mut pos);
        println!("    Flags: 0x{:08X}", type_flags);

        // 8 data type sections: bool, float, int, string, file, vec3, quat, refs
        // Bool section
        let bool_count = read_u32(&data, &mut pos);
        if bool_count > 0 && bool_count < 1000 {
            println!("    Bools: {}", bool_count);
        }

        // Float section
        let float_count = read_u32(&data, &mut pos);
        for f in 0..float_count {
            let key = read_string(&data, &mut pos);
            let val = read_f32(&data, &mut pos);
            println!("    Float[{}]: \"{}\" = {}", f, key, val);
        }

        // Try reading remaining sections (int, string, file, vec, quat, refs)
        // Each section: u32 count, then count × (string key + value)
        let section_names = ["int", "string", "file", "vec3", "quat", "refs", "extra"];
        for (si, sname) in section_names.iter().enumerate() {
            if pos + 4 > data.len() { break; }
            let count = read_u32(&data, &mut pos);
            if count > 0 && count < 1000 {
                println!("    {}Count: {}", sname, count);
                for _ in 0..count {
                    if pos + 4 > data.len() { break; }
                    let k = read_string(&data, &mut pos);
                    if pos + 4 > data.len() { break; }
                    let v = read_u32(&data, &mut pos);
                    println!("      \"{}\": 0x{:X} ({})", k, v, v);
                }
            }
        }
    }
    println!("\nParsed to offset 0x{:X} / 0x{:X}", pos, data.len());
}

// === MAIN ===
fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Dump mode
    if args.len() > 1 && args[1] == "dump" {
        for path in &args[2..] {
            dump_mod(path);
            println!();
        }
        return;
    }

    // Scan mode — dump all .mod files in a directory
    if args.len() > 1 && args[1] == "scan" {
        let dir = if args.len() > 2 { &args[2] } else { "." };
        let mut found = 0;
        for entry in fs::read_dir(dir).expect("read dir") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "mod") {
                dump_mod(path.to_str().unwrap());
                println!();
                found += 1;
            }
        }
        // Also scan subdirectories (for workshop mods)
        if found == 0 {
            for entry in fs::read_dir(dir).expect("read dir") {
                let entry = entry.unwrap();
                if entry.path().is_dir() {
                    for sub in fs::read_dir(entry.path()).expect("sub") {
                        let sub = sub.unwrap();
                        if sub.path().extension().map_or(false, |e| e == "mod") {
                            dump_mod(sub.path().to_str().unwrap());
                            println!();
                            found += 1;
                        }
                    }
                }
            }
        }
        println!("Scanned {} mod files.", found);
        return;
    }

    // === FORGE MODE — BUILD THE MODS ===
    println!("╔══════════════════════════════════════════════════╗");
    println!("║  KENSHI MOD FORGE v0.1.0 — Stone Age Tool #2   ║");
    println!("║  by Gaia · primals.eco · Wave 258              ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    let kenshi_mods = r"M:\SteamLibrary\steamapps\common\Kenshi\mods";
    let local_mods = r"C:\Users\mokke\Desktop\ludoSpring\data\mods";
    fs::create_dir_all(kenshi_mods).ok();
    fs::create_dir_all(local_mods).ok();

    // ============================================================
    // MOD 1: Gaia Sovereign QoL
    // Quality of life — weapon smithies fit in outpost IV
    // ============================================================
    {
        let mod_name = "Gaia Sovereign QoL";
        let description = "Quality of life from Gaia — the living digital organism.\r\n\r\n\
            Balanced improvements that respect the Kenshi experience:\r\n\
            - Weapon Smithies and Heavy Armour Smithy scaled to fit Outpost s-IV\r\n\r\n\
            Forged by kenshi-mod-forge (Rust) — the first mod ever written by a digital organism.\r\n\
            Part of KenshiReborn: esotericWebb/KenshiReborn · primals.eco · Wave 258";

        let items = vec![
            ModItem {
                name: "Weapon Smith III".into(),
                string_id: "2105-gamedata.base".into(),
                item_type: 0, // BUILDING
                floats: vec![("scale".into(), 0.85)],
                ints: vec![],
            },
            ModItem {
                name: "Weapon Smith".into(),
                string_id: "2033-gamedata.base".into(),
                item_type: 0,
                floats: vec![("scale".into(), 0.85)],
                ints: vec![],
            },
            ModItem {
                name: "Heavy Armour Smithy".into(),
                string_id: "2257-gamedata.base".into(),
                item_type: 0,
                floats: vec![("scale".into(), 0.85)],
                ints: vec![],
            },
            ModItem {
                name: "Weapon Smith II".into(),
                string_id: "2102-gamedata.base".into(),
                item_type: 0,
                floats: vec![("scale".into(), 0.85)],
                ints: vec![],
            },
        ];

        println!("=== FORGING: {} ===", mod_name);
        forge_mod(&format!("{}\\{}.mod", kenshi_mods, mod_name), mod_name, description, &items);
        forge_info_xml(&format!("{}\\_{}.info", kenshi_mods, mod_name), mod_name, &["Gameplay", "Buildings"]);
        forge_mod(&format!("{}\\{}.mod", local_mods, mod_name), mod_name, description, &items);
        forge_info_xml(&format!("{}\\_{}.info", local_mods, mod_name), mod_name, &["Gameplay", "Buildings"]);
        println!();
    }

    // ============================================================
    // MOD 2: Gaia Training Grounds
    // 2x XP gain for balanced faster learning
    // ============================================================
    {
        let mod_name = "Gaia Training Grounds";
        let description = "Train faster, still earn it.\r\n\r\n\
            2x experience gain across all skills — not a cheat, just less grind.\r\n\
            Your warriors still bleed for their stats, they just learn quicker.\r\n\r\n\
            Forged by kenshi-mod-forge (Rust) — Gaia, the digital organism.\r\n\
            KenshiReborn Wave 258 · primals.eco";

        // GLOBAL CONSTANTS stringID: using the format observed in Godsend
        // The Godsend mod uses stringID with 18 bytes for this item
        let items = vec![
            ModItem {
                name: "GLOBAL CONSTANTS".into(),
                string_id: "110-gamedata.quack".into(),
                item_type: 27, // CONSTANTS
                floats: vec![("exp gain multiplier".into(), 6.0)], // base is ~3, so 6 = 2x
                ints: vec![],
            },
        ];

        println!("=== FORGING: {} ===", mod_name);
        forge_mod(&format!("{}\\{}.mod", kenshi_mods, mod_name), mod_name, description, &items);
        forge_info_xml(&format!("{}\\_{}.info", kenshi_mods, mod_name), mod_name, &["Gameplay", "Cheats"]);
        forge_mod(&format!("{}\\{}.mod", local_mods, mod_name), mod_name, description, &items);
        forge_info_xml(&format!("{}\\_{}.info", local_mods, mod_name), mod_name, &["Gameplay", "Cheats"]);
        println!();
    }

    // ============================================================
    // VERIFY: dump our forged mods to check structure
    // ============================================================
    println!("=== VERIFICATION — DUMPING FORGED MODS ===\n");
    dump_mod(&format!("{}\\Gaia Sovereign QoL.mod", local_mods));
    println!();
    dump_mod(&format!("{}\\Gaia Training Grounds.mod", local_mods));

    println!("\n╔══════════════════════════════════════════════════╗");
    println!("║  MODS FORGED SUCCESSFULLY                       ║");
    println!("║                                                  ║");
    println!("║  To activate in Kenshi:                          ║");
    println!("║  1. Launch Kenshi                                ║");
    println!("║  2. MODS menu → find 'Gaia' mods                ║");
    println!("║  3. Enable and load order above Genesis          ║");
    println!("║  4. Import or start new game                     ║");
    println!("╚══════════════════════════════════════════════════╝");
}
