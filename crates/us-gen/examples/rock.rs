use us_gen::*;
fn main() {
    for s in 1..=12u64 {
        let t = generate(Preset::Mesa, s);
        if let Relief::Mesa(m) = t.terrain.relief() {
            let k = |r: Rock| m.formations.iter().filter(|f| f.kind == r).count();
            println!(
                "seed {s}: mesas {} buttes {} spires {} lots {}",
                k(Rock::Mesa),
                k(Rock::Butte),
                k(Rock::Spire),
                t.lots.len()
            );
        }
    }
}
