extern crate tcod;
extern crate rand;


use tcod::console::*;
use tcod::colors::{self, Color};
use tcod::map::{Map as FovMap, FovAlgorithm};

use mapgen::*;

mod mapgen;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

const LIMIT_FPS: i32 = 20;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Shadow; // default FOV algorithm
const FOV_LIGHT_WALLS: bool = true; // light walls or not
const TORCH_RADIUS: i32 = 200;

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

/// This is a generic object: the player, a monster, an item, the stairs...
/// It's always represented by a character on screen.
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, color: Color) -> Self {
        Object {
            x: x,
            y: y,
            char: char,
            color: color,
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
              fov_map: &mut FovMap,
              fov_recompute: bool) {
    if fov_recompute {
        // recompute FOV if needed (the player moved or something)
        let player = &objects[0];
        fov_map.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);

        // go through all tiles, and set their background color
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let visible = fov_map.is_in_fov(x, y);
                let wall = map[x as usize][y as usize].block_sight;
                let color = match (visible, wall) {
                    // outside of field of view:
                    (false, true) => COLOR_DARK_WALL,
                    (false, false) => COLOR_DARK_GROUND,
                    // inside fov:
                    (true, true) => COLOR_LIGHT_WALL,
                    (true, false) => COLOR_LIGHT_GROUND,
                };

                let explored = &mut map[x as usize][y as usize].explored;
                if visible {
                    // since it's visible, explore it
                    *explored = true;
                }
                if *explored {
                    // show explored tiles only (any visible tile is explored already)
                    con.set_char_background(x, y, color, BackgroundFlag::Set);
                }
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

fn handle_keys(root: &mut Root, player: &mut Object, map: &Map) -> bool {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;

    let key = root.wait_for_keypress(true);
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
        Key { code: Up, .. } => player.move_by(0, -1, map),
        Key { code: Down, .. } => player.move_by(0, 1, map),
        Key { code: Left, .. } => player.move_by(-1, 0, map),
        Key { code: Right, .. } => player.move_by(1, 0, map),

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
    let player = Object::new(player_x, player_y, '@', colors::WHITE);

    // the list of objects with those two
    let mut objects = [player];

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

    // force FOV "recompute" first time through the game loop
    let mut previous_player_position = (-1, -1);

    while !root.window_closed() {
        // render the screen
        let fov_recompute = previous_player_position != (objects[0].x, objects[0].y);
        render_all(&mut root,
                   &mut con,
                   &objects,
                   &mut map,
                   &mut fov_map,
                   fov_recompute);

        root.flush();

        // erase all objects at their old locations, before they move
        for object in &objects {
            object.clear(&mut con)
        }

        // handle keys and exit game if needed
        let player = &mut objects[0];
        previous_player_position = (player.x, player.y);
        let exit = handle_keys(&mut root, player, &map);
        if exit {
            break;
        }
    }
}
