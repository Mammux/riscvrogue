use alloc::vec;
use alloc::vec::Vec;

use rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use crate::engine::{BTerm, GameState, TickResult};
use crate::input::GameKey;
use crate::io::Console;

const W: usize = 60;
const H: usize = 30;

#[derive(Clone, Copy)]
pub struct Monster {
    x: usize,
    y: usize,
    hp: i32,
    attack: i32,
}

impl Monster {
    fn new(x: usize, y: usize) -> Self {
        Self {
            x,
            y,
            hp: 5,
            attack: 2,
        }
    }

    fn is_alive(&self) -> bool {
        self.hp > 0
    }
}
const GRID_W: usize = 3;
const GRID_H: usize = 3;
const MIN_ROOM_W: i32 = 4;
const MIN_ROOM_H: i32 = 3;
const LOS_RADIUS: i32 = 10;

const TILE_ROCK: u8 = b' ';
const TILE_WALL: u8 = b'#';
const TILE_ROOM: u8 = b'.';
const TILE_CORRIDOR: u8 = b',';
const TILE_DOOR: u8 = b'+';

pub struct DungeonState {
    tiles: Vec<u8>,
    explored: Vec<bool>,
    px: usize,
    py: usize,
    hit_points: i32,
    strength: i32,
    xp: i32,
    monsters: Vec<Monster>,
    options: DisplayOptions,
    font_dirty: bool,
}

impl DungeonState {
    pub fn new() -> Self {
        Self::new_with_seed(0x00C0_FFEE_F00D_BAAD)
    }

    pub fn new_with_seed(seed: u64) -> Self {
        let mut tiles = vec![TILE_ROCK; W * H];
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
        let rooms = generate_nethackish_rooms(&mut tiles, &mut rng);

        add_walls_around_open_tiles(&mut tiles);

        let (px, py) = rooms
            .first()
            .map(Room::center)
            .unwrap_or((2, 2));

        let monsters = spawn_monsters(&rooms, &mut rng);

        Self {
            tiles,
            explored: vec![false; W * H],
            px: px as usize,
            py: py as usize,
            hit_points: 20,
            strength: 10,
            xp: 0,
            monsters,
            options: DisplayOptions::default(),
            font_dirty: true,
        }
    }

    fn tile(&self, x: usize, y: usize) -> u8 {
        self.tiles[y * W + x]
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return false;
        }
        matches!(
            self.tiles[(y as usize) * W + (x as usize)],
            TILE_ROOM | TILE_CORRIDOR | TILE_DOOR
        )
    }

    fn render<C: Console + ?Sized>(&self, ctx: &mut BTerm<'_, C>, visibility: &[bool]) {
        ctx.cls();
        ctx.print("riscvrogue -- dungeon sample state\n\n");

        for y in 0..H {
            for x in 0..W {
                let index = y * W + x;
                let explored = self.explored[index];
                let visible = visibility[y * W + x];

                if !explored {
                    self.draw_tile(ctx, b' ', 37);
                    continue;
                }

                if x == self.px && y == self.py {
                    self.draw_tile(ctx, b'@', 93);
                } else if let Some(_monster) = self.monsters.iter().find(|m| m.x == x && m.y == y && m.is_alive()) {
                    self.draw_tile(ctx, b'M', 91);
                } else {
                    self.draw_map_tile(ctx, x, y, self.tile(x, y), visible);
                }
            }
            ctx.put_char(b'\n');
        }

        ctx.print("\nMove: h/j/k/l, arrows, keypad 1-9   Quit: q\n");
        crate::cprint!(ctx.console_mut(), "HP: {}  STR: {}  XP: {}\n", self.hit_points, self.strength, self.xp);
        ctx.print("Press O for options");

        if self.options.menu_open {
            ctx.print("\n=== Options ===\n");
            ctx.print("F: font       ");
            ctx.print(self.options.font.label());
            ctx.put_char(b'\n');
            ctx.print("G: graphics   ");
            ctx.print(if self.options.graphical_tiles {
                "on"
            } else {
                "off"
            });
            ctx.put_char(b'\n');
            ctx.print("C: color      ");
            ctx.print(if self.options.color {
                "on"
            } else {
                "off"
            });
            ctx.put_char(b'\n');
            ctx.print("O: close options\n");
        }
    }

    fn render_dead<C: Console + ?Sized>(&self, ctx: &mut BTerm<'_, C>) {
        ctx.cls();
        ctx.print("You are dead.\n\n");
        crate::cprint!(ctx.console_mut(), "Final stats -> HP: {}  STR: {}\n\n", self.hit_points, self.strength);
        ctx.print("Press SPACE to leave the game.\n");
    }

    fn draw_map_tile<C: Console + ?Sized>(&self, ctx: &mut BTerm<'_, C>, x: usize, y: usize, tile: u8, visible: bool) {
        if self.options.graphical_tiles {
            match tile {
                TILE_WALL => self.draw_tile(ctx, self.wall_glyph(x, y), 37),
                TILE_ROOM => {
                    let color = if self.options.color && !visible { 32 } else { 92 };
                    self.draw_tile(ctx, 0xFA, color)
                }
                TILE_DOOR => self.draw_tile(ctx, 0xFE, 93),
                TILE_CORRIDOR => {
                    let glyph = if visible { 0xB1 } else { 0xB0 };
                    self.draw_tile(ctx, glyph, 96)
                }
                TILE_ROCK => self.draw_tile(ctx, b' ', 37),
                _ => self.draw_tile(ctx, tile, 37),
            }
        } else {
            match tile {
                TILE_WALL => self.draw_tile(ctx, b'#', 37),
                TILE_ROOM => {
                    let color = if self.options.color && !visible { 32 } else { 92 };
                    self.draw_tile(ctx, b'.', color)
                }
                TILE_DOOR => self.draw_tile(ctx, b'+', 93),
                TILE_CORRIDOR => {
                    self.draw_tile(ctx, b',', 96)
                }
                TILE_ROCK => self.draw_tile(ctx, b' ', 37),
                _ => self.draw_tile(ctx, tile, 37),
            }
        }
    }

    fn compute_visibility(&self) -> Vec<bool> {
        let mut visible = vec![false; W * H];
        let px = self.px as i32;
        let py = self.py as i32;

        for y in 0..(H as i32) {
            for x in 0..(W as i32) {
                let dx = x - px;
                let dy = y - py;
                if dx * dx + dy * dy > LOS_RADIUS * LOS_RADIUS {
                    continue;
                }
                if self.has_line_of_sight(px, py, x, y) {
                    visible[(y as usize) * W + (x as usize)] = true;
                }
            }
        }

        visible[self.py * W + self.px] = true;
        visible
    }

    fn update_explored(&mut self, visibility: &[bool]) {
        for (index, &is_visible) in visibility.iter().enumerate() {
            if is_visible {
                self.explored[index] = true;
            }
        }
    }

    fn has_line_of_sight(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        let points = bresenham_line(x0, y0, x1, y1);
        for (index, &(x, y)) in points.iter().enumerate() {
            if index == 0 {
                continue;
            }
            if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
                return false;
            }

            let tile = self.tiles[(y as usize) * W + (x as usize)];
            if is_opaque(tile) {
                // Target opaque tile is still visible; beyond it is not.
                return x == x1 && y == y1;
            }
        }
        true
    }

    fn wall_glyph(&self, x: usize, y: usize) -> u8 {
        let north = self.is_wall(x as i32, y as i32 - 1);
        let south = self.is_wall(x as i32, y as i32 + 1);
        let west = self.is_wall(x as i32 - 1, y as i32);
        let east = self.is_wall(x as i32 + 1, y as i32);

        // Edge-aware overrides:
        // Keep genuine corners on the boundary, but collapse only the
        // edge-facing T-junction artifacts to straight lines.
        if y == 0 {
            if south && east && west {
                return 0xC4; // ─
            }
            if south && east {
                return 0xDA; // ┌
            }
            if south && west {
                return 0xBF; // ┐
            }
        }
        if y == (H - 1) {
            if north && east && west {
                return 0xC4; // ─
            }
            if north && east {
                return 0xC0; // └
            }
            if north && west {
                return 0xD9; // ┘
            }
        }
        if x == 0 {
            if north && south && east {
                return 0xB3; // │
            }
            if south && east {
                return 0xDA; // ┌
            }
            if north && east {
                return 0xC0; // └
            }
        }
        if x == (W - 1) {
            if north && south && west {
                return 0xB3; // │
            }
            if south && west {
                return 0xBF; // ┐
            }
            if north && west {
                return 0xD9; // ┘
            }
        }

        match (north, south, west, east) {
            (true, true, true, true) => 0xC5,  // ┼
            (true, true, true, false) => 0xB4, // ┤
            (true, true, false, true) => 0xC3, // ├
            (true, false, true, true) => 0xC1, // ┴
            (false, true, true, true) => 0xC2, // ┬
            (true, true, false, false) => 0xB3, // │
            (false, false, true, true) => 0xC4, // ─
            (true, false, false, true) => 0xC0, // └
            (true, false, true, false) => 0xD9, // ┘
            (false, true, false, true) => 0xDA, // ┌
            (false, true, true, false) => 0xBF, // ┐
            (true, false, false, false) | (false, true, false, false) => 0xB3,
            (false, false, true, false) | (false, false, false, true) => 0xC4,
            _ => 0xB1,
        }
    }

    fn is_wall(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return true;
        }
        matches!(self.tiles[(y as usize) * W + (x as usize)], TILE_WALL | TILE_DOOR)
    }

    fn draw_tile<C: Console + ?Sized>(&self, ctx: &mut BTerm<'_, C>, byte: u8, color_code: u8) {
        if self.options.color {
            ctx.set_sgr(color_code);
            ctx.put_char(byte);
            ctx.reset_style();
        } else {
            ctx.put_char(byte);
        }
    }

    fn apply_options<C: Console + ?Sized>(&mut self, ctx: &mut BTerm<'_, C>) {
        if self.font_dirty {
            ctx.set_font(self.options.font.kernel_index());
            self.font_dirty = false;
        }
    }

    fn is_dead(&self) -> bool {
        self.hit_points <= 0
    }

    fn apply_damage(&mut self, amount: i32) {
        self.hit_points = (self.hit_points - amount).max(0);
    }

    fn attack_monster(&mut self, monster_index: usize) {
        if monster_index >= self.monsters.len() {
            return;
        }

        let damage = (self.strength / 2).max(1);
        self.monsters[monster_index].hp -= damage;

        if self.monsters[monster_index].hp <= 0 {
            self.xp += 10;
        } else {
            let counter_damage = self.monsters[monster_index].attack;
            self.apply_damage(counter_damage);
        }
    }

    fn find_monster_at(&self, x: usize, y: usize) -> Option<usize> {
        self.monsters
            .iter()
            .position(|m| m.x == x && m.y == y && m.is_alive())
    }

    fn on_options_key<C: Console + ?Sized>(&mut self, key: GameKey, ctx: &mut BTerm<'_, C>) -> TickResult {
        match key {
            GameKey::Quit => {
                ctx.print("\nLeaving dungeon...\n");
                TickResult::Quit
            }
            GameKey::Options => {
                self.options.menu_open = false;
                TickResult::Continue
            }
            GameKey::Char(ch) => {
                match ch {
                    b'f' | b'F' => {
                        self.options.font = self.options.font.next();
                        self.font_dirty = true;
                    }
                    b'g' | b'G' => {
                        self.options.graphical_tiles = !self.options.graphical_tiles;
                    }
                    b'c' | b'C' => {
                        self.options.color = !self.options.color;
                    }
                    _ => {}
                }
                TickResult::Continue
            }
            _ => TickResult::Continue,
        }
    }
}

impl GameState for DungeonState {
    fn tick<C: Console + ?Sized>(&mut self, ctx: &mut BTerm<'_, C>) -> TickResult {
        self.apply_options(ctx);

        if self.is_dead() {
            self.render_dead(ctx);
            return match ctx.key() {
                GameKey::Char(b' ') => TickResult::Quit,
                _ => TickResult::Continue,
            };
        }

        let visibility = self.compute_visibility();
        self.update_explored(&visibility);
        self.render(ctx, &visibility);

        let key = ctx.key();

        if self.options.menu_open {
            return self.on_options_key(key, ctx);
        }

        match key {
            GameKey::Quit => {
                ctx.print("\nLeaving dungeon...\n");
                TickResult::Quit
            }
            GameKey::Options => {
                self.options.menu_open = true;
                TickResult::Continue
            }
            GameKey::Move { dx, dy } => {
                let nx = self.px as i32 + dx;
                let ny = self.py as i32 + dy;
                if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
                    TickResult::Continue
                } else {
                    let nx_usize = nx as usize;
                    let ny_usize = ny as usize;

                    if let Some(monster_index) = self.find_monster_at(nx_usize, ny_usize) {
                        self.attack_monster(monster_index);
                    } else if self.is_walkable(nx, ny) {
                        self.px = nx_usize;
                        self.py = ny_usize;
                    }
                    TickResult::Continue
                }
            }
            GameKey::Char(_) => TickResult::Continue,
            GameKey::Unknown => TickResult::Continue,
        }
    }
}

#[derive(Clone, Copy)]
struct DisplayOptions {
    font: FontChoice,
    graphical_tiles: bool,
    color: bool,
    menu_open: bool,
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            font: FontChoice::Regular8x8,
            graphical_tiles: true,
            color: true,
            menu_open: false,
        }
    }
}

#[derive(Clone, Copy)]
enum FontChoice {
    Regular8x8,
    Bold8x8,
    Regular9x14,
}

impl FontChoice {
    fn next(self) -> Self {
        match self {
            FontChoice::Regular8x8 => FontChoice::Bold8x8,
            FontChoice::Bold8x8 => FontChoice::Regular9x14,
            FontChoice::Regular9x14 => FontChoice::Regular8x8,
        }
    }

    fn label(self) -> &'static str {
        match self {
            FontChoice::Regular8x8 => "IBM437 8x8 regular",
            FontChoice::Bold8x8 => "IBM437 8x8 bold",
            FontChoice::Regular9x14 => "IBM437 9x14 regular",
        }
    }

    fn kernel_index(self) -> u8 {
        match self {
            FontChoice::Regular8x8 => 0,
            FontChoice::Bold8x8 => 1,
            FontChoice::Regular9x14 => 2,
        }
    }
}

#[derive(Clone, Copy)]
struct Room {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

#[derive(Clone, Copy)]
struct DoorEndpoint {
    wall_x: i32,
    wall_y: i32,
    out_dx: i32,
    out_dy: i32,
}

impl Room {
    fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    fn center(&self) -> (i32, i32) {
        ((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
    }

    fn intersects_with_gap(&self, other: &Room, gap: i32) -> bool {
        self.x1 <= (other.x2 + gap)
            && self.x2 >= (other.x1 - gap)
            && self.y1 <= (other.y2 + gap)
            && self.y2 >= (other.y1 - gap)
    }

    fn random_door_towards<R: RngCore>(&self, target: &Room, rng: &mut R) -> DoorEndpoint {
        let (sx, sy) = self.center();
        let (tx, ty) = target.center();
        let dx = tx - sx;
        let dy = ty - sy;

        if dx.abs() >= dy.abs() {
            let y = rand_range_i32(rng, self.y1 + 1, self.y2);
            if dx >= 0 {
                DoorEndpoint {
                    wall_x: self.x2,
                    wall_y: y,
                    out_dx: 1,
                    out_dy: 0,
                }
            } else {
                DoorEndpoint {
                    wall_x: self.x1,
                    wall_y: y,
                    out_dx: -1,
                    out_dy: 0,
                }
            }
        } else {
            let x = rand_range_i32(rng, self.x1 + 1, self.x2);
            if dy >= 0 {
                DoorEndpoint {
                    wall_x: x,
                    wall_y: self.y2,
                    out_dx: 0,
                    out_dy: 1,
                }
            } else {
                DoorEndpoint {
                    wall_x: x,
                    wall_y: self.y1,
                    out_dx: 0,
                    out_dy: -1,
                }
            }
        }
    }
}

fn spawn_monsters<R: RngCore>(rooms: &[Room], rng: &mut R) -> Vec<Monster> {
    let mut monsters = Vec::new();

    for room in rooms.iter() {
        if (rng.next_u32() % 100) < 60 {
            let x = rand_range_i32(rng, room.x1 + 1, room.x2) as usize;
            let y = rand_range_i32(rng, room.y1 + 1, room.y2) as usize;
            monsters.push(Monster::new(x, y));
        }
    }

    monsters
}

fn generate_nethackish_rooms<R: RngCore>(tiles: &mut [u8], rng: &mut R) -> Vec<Room> {
    let mut rooms: Vec<Room> = Vec::new();
    let mut grid_slots: [Option<usize>; GRID_W * GRID_H] = [None; GRID_W * GRID_H];

    let cell_w = W as i32 / GRID_W as i32;
    let cell_h = H as i32 / GRID_H as i32;

    for gy in 0..GRID_H {
        for gx in 0..GRID_W {
            // NetHack-like: some rooms missing.
            if (rng.next_u32() % 100) < 28 {
                continue;
            }

            let region_x0 = gx as i32 * cell_w + 1;
            let region_y0 = gy as i32 * cell_h + 1;
            let region_x1 = if gx == GRID_W - 1 {
                W as i32 - 2
            } else {
                (gx as i32 + 1) * cell_w - 2
            };
            let region_y1 = if gy == GRID_H - 1 {
                H as i32 - 2
            } else {
                (gy as i32 + 1) * cell_h - 2
            };

            let max_w = (region_x1 - region_x0 - 1).max(MIN_ROOM_W);
            let max_h = (region_y1 - region_y0 - 1).max(MIN_ROOM_H);
            if max_w < MIN_ROOM_W || max_h < MIN_ROOM_H {
                continue;
            }

            let room_w = rand_range_i32(rng, MIN_ROOM_W, max_w + 1);
            let room_h = rand_range_i32(rng, MIN_ROOM_H, max_h + 1);
            let room_x = rand_range_i32(rng, region_x0, region_x1 - room_w + 2);
            let room_y = rand_range_i32(rng, region_y0, region_y1 - room_h + 2);

            let room = Room::new(room_x, room_y, room_w, room_h);

            // Hard guarantee: rooms are never adjacent (at least one tile gap).
            if rooms.iter().any(|other| room.intersects_with_gap(other, 1)) {
                continue;
            }

            carve_room(tiles, &room);
            let room_index = rooms.len();
            rooms.push(room);
            grid_slots[gy * GRID_W + gx] = Some(room_index);
        }
    }

    // Ensure at least one room exists.
    if rooms.is_empty() {
        let fallback = Room::new((W as i32 / 2) - 4, (H as i32 / 2) - 3, 8, 6);
        carve_room(tiles, &fallback);
        rooms.push(fallback);
        return rooms;
    }

    connect_room_grid(tiles, &rooms, &grid_slots, rng);
    rooms
}

fn connect_room_grid<R: RngCore>(
    tiles: &mut [u8],
    rooms: &[Room],
    grid_slots: &[Option<usize>; GRID_W * GRID_H],
    rng: &mut R,
) {
    let mut edges: Vec<(usize, usize)> = Vec::new();

    for gy in 0..GRID_H {
        for gx in 0..GRID_W {
            let idx = gy * GRID_W + gx;
            let Some(a) = grid_slots[idx] else { continue };

            if gx + 1 < GRID_W {
                if let Some(b) = grid_slots[gy * GRID_W + (gx + 1)] {
                    edges.push((a, b));
                }
            }
            if gy + 1 < GRID_H {
                if let Some(b) = grid_slots[(gy + 1) * GRID_W + gx] {
                    edges.push((a, b));
                }
            }
        }
    }

    shuffle_edges(&mut edges, rng);

    // Spanning connectivity first.
    let mut uf = UnionFind::new(rooms.len());
    let mut used: Vec<(usize, usize)> = Vec::new();
    for &(a, b) in &edges {
        if uf.union(a, b) {
            used.push((a, b));
        }
    }

    // Hard guarantee: if the grid graph ended up split, bridge the
    // remaining components by linking nearest room pairs.
    while !all_rooms_connected(&mut uf, rooms.len()) {
        let mut best_pair: Option<(usize, usize, i32)> = None;

        for a in 0..rooms.len() {
            for b in (a + 1)..rooms.len() {
                if uf.find(a) == uf.find(b) {
                    continue;
                }
                let distance = room_center_distance(&rooms[a], &rooms[b]);
                match best_pair {
                    Some((_, _, best_distance)) if distance >= best_distance => {}
                    _ => best_pair = Some((a, b, distance)),
                }
            }
        }

        let Some((a, b, _)) = best_pair else { break };
        if uf.union(a, b) {
            used.push((a, b));
        }
    }

    // Add a few extra links for loops (NetHack-ish feel).
    for &(a, b) in &edges {
        if used.iter().any(|&(x, y)| (x == a && y == b) || (x == b && y == a)) {
            continue;
        }
        if (rng.next_u32() % 100) < 30 {
            used.push((a, b));
        }
    }

    for (a, b) in used {
        carve_room_link(tiles, &rooms[a], &rooms[b], rng);
    }
}

fn all_rooms_connected(uf: &mut UnionFind, room_count: usize) -> bool {
    if room_count <= 1 {
        return true;
    }
    let root0 = uf.find(0);
    for index in 1..room_count {
        if uf.find(index) != root0 {
            return false;
        }
    }
    true
}

fn room_center_distance(a: &Room, b: &Room) -> i32 {
    let (ax, ay) = a.center();
    let (bx, by) = b.center();
    (ax - bx).abs() + (ay - by).abs()
}

fn carve_room_link<R: RngCore>(tiles: &mut [u8], a: &Room, b: &Room, rng: &mut R) {
    let a_door = a.random_door_towards(b, rng);
    let b_door = b.random_door_towards(a, rng);

    set_door_tile(tiles, a_door.wall_x, a_door.wall_y);
    set_door_tile(tiles, b_door.wall_x, b_door.wall_y);

    // Corridor starts just outside each room wall (single contact with room),
    // then moves outward before turning to avoid wall-hugging runs.
    let mut x1 = a_door.wall_x + a_door.out_dx;
    let mut y1 = a_door.wall_y + a_door.out_dy;
    let mut x2 = b_door.wall_x + b_door.out_dx;
    let mut y2 = b_door.wall_y + b_door.out_dy;

    set_corridor_floor(tiles, x1, y1);
    set_corridor_floor(tiles, x2, y2);

    // One extra step away from each wall before the long run.
    x1 += a_door.out_dx;
    y1 += a_door.out_dy;
    x2 += b_door.out_dx;
    y2 += b_door.out_dy;

    set_corridor_floor(tiles, x1, y1);
    set_corridor_floor(tiles, x2, y2);

    if (rng.next_u32() & 1) == 0 {
        carve_h_tunnel(tiles, x1, x2, y1);
        carve_v_tunnel(tiles, y1, y2, x2);
    } else {
        carve_v_tunnel(tiles, y1, y2, x1);
        carve_h_tunnel(tiles, x1, x2, y2);
    }
}

fn shuffle_edges<R: RngCore>(edges: &mut [(usize, usize)], rng: &mut R) {
    let len = edges.len();
    for i in (1..len).rev() {
        let j = (rng.next_u32() as usize) % (i + 1);
        edges.swap(i, j);
    }
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        let mut parent = Vec::with_capacity(size);
        for index in 0..size {
            parent.push(index);
        }
        Self {
            parent,
            rank: vec![0; size],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        let p = self.parent[x];
        if p != x {
            let root = self.find(p);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return false;
        }

        if self.rank[ra] < self.rank[rb] {
            self.parent[ra] = rb;
        } else if self.rank[ra] > self.rank[rb] {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] = self.rank[ra].saturating_add(1);
        }
        true
    }
}

fn carve_room(tiles: &mut [u8], room: &Room) {
    for y in (room.y1 + 1)..room.y2 {
        for x in (room.x1 + 1)..room.x2 {
            set_room_floor(tiles, x, y);
        }
    }
}

fn carve_h_tunnel(tiles: &mut [u8], x1: i32, x2: i32, y: i32) {
    let min_x = x1.min(x2);
    let max_x = x1.max(x2);
    for x in min_x..=max_x {
        set_corridor_floor(tiles, x, y);
    }
}

fn carve_v_tunnel(tiles: &mut [u8], y1: i32, y2: i32, x: i32) {
    let min_y = y1.min(y2);
    let max_y = y1.max(y2);
    for y in min_y..=max_y {
        set_corridor_floor(tiles, x, y);
    }
}

fn set_room_floor(tiles: &mut [u8], x: i32, y: i32) {
    if x <= 0 || y <= 0 || x >= (W as i32 - 1) || y >= (H as i32 - 1) {
        return;
    }
    tiles[(y as usize) * W + (x as usize)] = TILE_ROOM;
}

fn set_corridor_floor(tiles: &mut [u8], x: i32, y: i32) {
    if x <= 0 || y <= 0 || x >= (W as i32 - 1) || y >= (H as i32 - 1) {
        return;
    }
    let index = (y as usize) * W + (x as usize);
    if tiles[index] == TILE_ROCK {
        tiles[index] = TILE_CORRIDOR;
    }
}

fn set_door_tile(tiles: &mut [u8], x: i32, y: i32) {
    if x <= 0 || y <= 0 || x >= (W as i32 - 1) || y >= (H as i32 - 1) {
        return;
    }
    let index = (y as usize) * W + (x as usize);
    if tiles[index] == TILE_ROCK || tiles[index] == TILE_WALL {
        tiles[index] = TILE_DOOR;
    }
}

fn bresenham_line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut points: Vec<(i32, i32)> = Vec::new();

    let mut x = x0;
    let mut y = y0;

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        points.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }

    points
}

fn is_opaque(tile: u8) -> bool {
    matches!(tile, TILE_WALL | TILE_ROCK)
}

fn add_walls_around_open_tiles(tiles: &mut [u8]) {
    let mut to_wall: Vec<usize> = Vec::new();

    for y in 1..(H - 1) {
        for x in 1..(W - 1) {
            let index = y * W + x;
            if tiles[index] != TILE_ROOM {
                continue;
            }

            for ny in (y - 1)..=(y + 1) {
                for nx in (x - 1)..=(x + 1) {
                    let nindex = ny * W + nx;
                    if tiles[nindex] == TILE_ROCK {
                        to_wall.push(nindex);
                    }
                }
            }
        }
    }

    for index in to_wall {
        tiles[index] = TILE_WALL;
    }
}

fn rand_range_i32<R: RngCore>(rng: &mut R, min: i32, max: i32) -> i32 {
    let span = (max - min) as u32;
    min + (rng.next_u32() % span) as i32
}
