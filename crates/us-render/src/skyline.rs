//! High-density towers with street podiums, chamfered upper volumes, glazed
//! floors and articulated crowns. Dimensions are in metres before lot fitting.

use crate::obj::{Obj, Rgb};
use crate::parts::Region;
use crate::rng::Rng;

const GLASS: Rgb = [98, 156, 204];
const SHOP_GLASS: Rgb = [64, 88, 118];

fn chamfered(x: f32, y: f32, corner: f32) -> [[f32; 2]; 8] {
    [
        [-x + corner, -y],
        [x - corner, -y],
        [x, -y + corner],
        [x, y - corner],
        [x - corner, y],
        [-x + corner, y],
        [-x, y - corner],
        [-x, -y + corner],
    ]
}

fn glazed_floors(
    o: &mut Obj,
    ring: &[[f32; 2]],
    start: f32,
    floors: usize,
    height: f32,
    frame: Rgb,
) {
    for i in 0..ring.len() {
        let [ax, ay] = ring[i];
        let [bx, by] = ring[(i + 1) % ring.len()];
        let (dx, dy) = (bx - ax, by - ay);
        let width = dx.hypot(dy);
        let bays = (width / 3.0).round().max(1.0) as usize;
        let at = |t| [ax + dx * t, ay + dy * t];
        for floor in 0..floors {
            let z = start + floor as f32 * height;
            let trim = if (floor + 1) % 8 == 0 {
                frame
            } else {
                [60, 73, 82]
            };
            o.wall_quad(at(0.05), at(0.95), z + 0.33, z + 0.65, trim);
            for bay in 0..bays {
                let low = (bay as f32 + 0.09) / bays as f32;
                let high = (bay as f32 + 0.91) / bays as f32;
                let glass = if floor == 0 && start == 0.0 {
                    SHOP_GLASS
                } else {
                    GLASS
                };
                o.wall_quad(at(low), at(high), z + 0.75, z + height * 0.89, glass);
            }
        }
    }
}

pub(crate) fn tower(region: Region, office: bool, level: u8, density: f32, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let peak = ((density - 0.78) / 0.22).clamp(0.0, 1.0);
    let variant = rng.below(3);
    let floors = if office {
        if level == 5 {
            28 + (peak * 10.0) as usize
        } else {
            17 + (peak * 5.0) as usize
        }
    } else if level == 5 {
        18 + (peak * 8.0) as usize
    } else {
        12 + (peak * 4.0) as usize
    };
    let storey = if office { 3.65 } else { 3.2 };
    let warm = matches!(region, Region::Sunbelt);
    let frame = if warm {
        [198, 177, 144]
    } else if variant == 0 {
        [175, 159, 133]
    } else {
        [132, 151, 159]
    };
    let metal = [178, 184, 182];
    let roof = [108, 115, 115];
    let broad = if variant == 1 { 12.8 } else { 11.8 };
    let podium = chamfered(broad, 9.7, 1.25);
    let stem = chamfered(broad - 2.1, 8.1, if variant == 2 { 2.4 } else { 1.2 });
    let upper = chamfered(broad - 4.15, 6.3, 1.2);
    let cap = chamfered(broad - 5.0, 5.3, 0.95);
    let tiers = [
        (&podium, 0, 3, [156, 151, 139]),
        (&stem, 3, floors * 3 / 4, frame),
        (&upper, floors * 3 / 4, floors, frame),
    ];
    for (outline, from, to, mat) in tiers {
        let z = from as f32 * storey;
        o.extrude(outline, z, to as f32 * storey, mat, roof);
        glazed_floors(&mut o, outline, z, to - from, storey, metal);
    }

    let lobby_y = -9.84;
    o.bx_top(
        -4.3,
        lobby_y - 1.65,
        3.55,
        4.3,
        lobby_y + 0.3,
        3.85,
        [63, 77, 86],
        metal,
    );
    for x in [-3.8, 3.8] {
        o.bx(
            x - 0.13,
            lobby_y - 1.35,
            0.0,
            x + 0.13,
            lobby_y - 1.02,
            3.55,
            metal,
        );
    }
    let top = floors as f32 * storey;
    o.extrude(&cap, top, top + 2.25, [89, 107, 116], metal);
    if variant == 0 {
        let crown = chamfered(3.5, 2.8, 0.65);
        o.loft(&cap, top + 2.25, &crown, top + 5.5, metal, Some(roof));
        o.bx(-0.2, -0.2, top + 5.5, 0.2, 0.2, top + 11.0, metal);
    } else {
        for x in [-2.85, 2.85] {
            o.bx_top(
                x - 1.0,
                -1.75,
                top + 2.25,
                x + 1.0,
                1.75,
                top + 3.4,
                roof,
                metal,
            );
        }
    }
    if !office {
        // Usable setbacks and protruding balconies soften the residential silhouette.
        for n in (4..floors * 3 / 4).step_by(2) {
            let z = n as f32 * storey + 0.1;
            for x in [-4.4, 4.4] {
                o.bx_top(x - 2.2, -9.25, z, x + 2.2, -8.1, z + 0.2, metal, roof);
                o.bx(
                    x - 2.2,
                    -9.25,
                    z + 0.2,
                    x + 2.2,
                    -9.13,
                    z + 1.1,
                    [113, 153, 161],
                );
            }
        }
    }
    o
}
