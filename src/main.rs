extern crate tcod;
extern crate rand;

use rand::distributions::{Normal, IndependentSample};
use tcod::console::*;
use tcod::colors::{self, Color, lerp};
use tcod::map::{Map as FovMap, FovAlgorithm};
use tcod::input::{self, Event, Key};

use mapgen::*;

mod mapgen;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

const LIMIT_FPS: i32 = 20;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Shadow;
const FOV_LIGHT_WALLS: bool = true; // light walls or not
const SIGHT_RADIUS: i32 = 100;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color {
    r: 130,
    g: 110,
    b: 50,
};
const COLOR_DARK_GROUND: Color = Color {
    r: 50,
    g: 50,
    b: 150,
};
const COLOR_LIGHT_GROUND: Color = Color {
    r: 200,
    g: 180,
    b: 50,
};

const COLOR_FOG_GROUND: Color = Color {
    r: 25,
    g: 25,
    b: 75,
};
const COLOR_FOG_WALL: Color = Color { r: 0, g: 0, b: 50 };

/// This is a generic object: the player, a monster, an item, the stairs...
/// It's always represented by a character on screen.
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    pub torch_distance: i32,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, color: Color) -> Self {
        Object {
            x: x,
            y: y,
            char: char,
            color: color,
            torch_distance: 0,
        }
    }

    /// move by the given amount, if the destination is not blocked
    pub fn move_by(&mut self, dx: i32, dy: i32, map: &Map) {
        if !map[(self.x + dx) as usize][(self.y + dy) as usize].blocked {
            self.x += dx;
            self.y += dy;
        }
    }

    /// set the color and then draw the character that represents this object at its position
    pub fn draw(&self, con: &mut Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    /// Erase the character that represents this object
    pub fn clear(&self, con: &mut Console) {
        con.put_char(self.x, self.y, ' ', BackgroundFlag::None);
    }
}

fn render_all(root: &mut Root,
              con: &mut Offscreen,
              objects: &[Object],
              map: &mut Map,
              fov_map: &mut FovMap) {
    // Compute lighting
    map.clear_light();
    for object in objects {
        if object.torch_distance > 0 {
            let torch_intensity_shift = Normal::new(0.0, 0.05)
                .ind_sample(&mut rand::thread_rng()) as f32;
            let td = object.torch_distance;
            fov_map.compute_fov(object.x, object.y, td, FOV_LIGHT_WALLS, FOV_ALGO);
            for y in (object.y - td)..(object.y + td + 1) {
                if y < 0 || y >= MAP_HEIGHT {
                    continue;
                }
                for x in (object.x - td)..(object.x + td + 1) {
                    if x < 0 || x >= MAP_WIDTH {
                        continue;
                    }
                    if fov_map.is_in_fov(x, y) {
                        let d = 1.0 - Map::distance(object.x, object.y, x, y) / (td as f32) +
                                torch_intensity_shift;
                        map[x as usize][y as usize].light_intensity += d;
                    }
                }
            }
        }
    }

    let player = &objects[0];
    fov_map.compute_fov(player.x, player.y, SIGHT_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);

    // go through all tiles, and set their background color
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = fov_map.is_in_fov(x, y);
            let wall = map[x as usize][y as usize].block_sight;
            let intensity = map[x as usize][y as usize]
                .light_intensity
                .min(1.0)
                .max(0.0);
            let color = match (visible, wall) {
                // outside of field of view:
                (false, true) => COLOR_FOG_WALL,
                (false, false) => COLOR_FOG_GROUND,
                // inside fov:
                (true, true) => lerp(COLOR_DARK_WALL, COLOR_LIGHT_WALL, intensity),
                (true, false) => lerp(COLOR_DARK_GROUND, COLOR_LIGHT_GROUND, intensity),
            };

            let explored = &mut map[x as usize][y as usize].explored;
            if visible && intensity > 0.0 {
                // since it's visible, explore it
                *explored = true;
            }
            if *explored {
                // show explored tiles only (any visible tile is explored already)
                con.set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    // draw all objects in the list
    for object in objects {
        if fov_map.is_in_fov(object.x, object.y) {
            object.draw(con);
        }
    }

    // blit the contents of "con" to the root console
    blit(con, (0, 0), (MAP_WIDTH, MAP_HEIGHT), root, (0, 0), 1.0, 1.0);
}

fn handle_keys(key: Key,
               root: &mut Root,
               player_idx: usize,
               objects: &mut Vec<Object>,
               map: &Map)
               -> bool {
    use tcod::input::KeyCode::*;

    match key {
        Key {
            code: Enter,
            alt: true,
            ..
        } => {
            let fullscreen = root.is_fullscreen();
            root.set_fullscreen(!fullscreen);
        }
        Key { code: Escape, .. } => return true,
        Key { code: Up, .. } => objects[player_idx].move_by(0, -1, map),
        Key { code: Down, .. } => objects[player_idx].move_by(0, 1, map),
        Key { code: Left, .. } => objects[player_idx].move_by(-1, 0, map),
        Key { code: Right, .. } => objects[player_idx].move_by(1, 0, map),
        Key { code: Spacebar, .. } => {
            let (x, y) = {
                let player = &objects[player_idx];
                (player.x, player.y)
            };
            let mut torch = Object::new(x, y, 'i', colors::COPPER);
            torch.torch_distance = 5;
            objects.push(torch);
        }

        _ => {}
    }
    false
}

fn main() {
    let mut root = Root::initializer()
        .font("media/arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Roguelike!")
        .init();

    tcod::system::set_fps(LIMIT_FPS);

    let mut con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);

    // generate map (at this point it's not drawn to the screen)
    let (mut map, (player_x, player_y)) = make_map();

    // create object representing the player
    // place the player inside the first room
    let mut player = Object::new(player_x, player_y, '@', colors::WHITE);
    player.torch_distance = 5;

    // the list of objects with those two
    let mut objects = vec![player];

    // create the FOV map, according to the generated map
    let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            fov_map.set(x,
                        y,
                        !map[x as usize][y as usize].block_sight,
                        !map[x as usize][y as usize].blocked);
        }
    }

    let mut key;

    while !root.window_closed() {
        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Key(k))) => key = k,
            _ => key = Default::default(),
        }

        // render the screen
        render_all(&mut root, &mut con, &objects, &mut map, &mut fov_map);

        root.flush();

        // erase all objects at their old locations, before they move
        for object in &objects {
            object.clear(&mut con)
        }

        // handle keys and exit game if needed
        let exit = handle_keys(key, &mut root, 0, &mut objects, &map);
        if exit {
            break;
        }
    }
}
