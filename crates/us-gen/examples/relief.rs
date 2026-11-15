//! Prints a coarse map of heights and steepness for a style, and how much ground can be built on.
use us_gen::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = Preset::from_name(&a[1]).unwrap();
    let seed: u64 = a[2].parse().unwrap();
    let t = generate(p, seed);
    let (mut flat, mut steep, mut sea, mut hi, mut lo) = (0, 0, 0, 0.0f64, 1e9f64);
    let n = 128;
    for j in 0..n {
        for i in 0..n {
            let q = pt(i as f64 * 32.0 + 16.0, j as f64 * 32.0 + 16.0);
            let h = t.height_at(q);
            hi = hi.max(h);
            lo = lo.min(h);
            if t.water.iter().any(|w| w.len() > 3 && inside(q, w)) { sea += 1; }
            else if t.slope_at(q) < 0.15 { flat += 1 } else { steep += 1 }
        }
    }
    println!("height {lo:.0}..{hi:.0} buildable {flat} steep {steep} water {sea} of {} cells; lots {}", n*n, t.lots.len());
    for j in (0..32).rev() {
        let mut s = String::new();
        for i in 0..64 {
            let q = pt(i as f64 * 64.0 + 32.0, j as f64 * 128.0 + 64.0);
            let h = t.height_at(q);
            let sl = t.slope_at(q);
            s.push(if t.water.iter().any(|w| w.len() > 3 && inside(q, w)) { '~' } else if sl > 0.5 { '#' } else if sl > 0.15 { '+' } else if h > 100.0 { 'H' } else if h > 50.0 { 'm' } else { '.' });
        }
        println!("{s}");
    }
}
fn inside(p: Pt, ring: &[Pt]) -> bool {
    let mut c = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x { c = !c; }
        j = i;
    }
    c
}
