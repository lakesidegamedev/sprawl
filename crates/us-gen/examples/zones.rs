use us_gen::{Preset, RockCoast, generate};
fn main() {
    let town = generate(Preset::Coast, 1);
    if let RockCoast::Coast(c) = town.terrain.rock_coast() {
        for v in &c.coves {
            println!(
                "cove c=({:.0},{:.0}) rho {:.0} harbour {}",
                v.c.x, v.c.y, v.rho, v.harbour
            );
        }
        for (x, y) in [(3000.0, 1730.0), (3040.0, 1730.0), (3100.0, 1730.0)] {
            println!(
                "({x},{y}) inland {:.1} height {:.1} edge(-30) {:.0}",
                c.inland(x, y),
                town.terrain.height(x, y),
                c.edge_x(y, -30.0)
            );
        }
        let (hs, hn) = c.horns(-30.0);
        println!("horns {:?} {:?}", (hs.x, hs.y), (hn.x, hn.y));
    }
}
