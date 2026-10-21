use std::fs::OpenOptions;
use std::io::Write;
use std::os::raw::{c_int, c_ulong};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TIOCGWINSZ: c_ulong = 0x5413;
const SIGHUP: c_int = 1;
const SIGINT: c_int = 2;
const SIGTERM: c_int = 15;
const SIGTTOU: c_int = 22;
const SIG_IGN: usize = 1;

const TICK: Duration = Duration::from_millis(150);
const SPRITE: &str = "(•‿•)";
const SPRITE_BLINK: &str = "(-‿-)";
const SPRITE_W: usize = 5;
const PHRASES: &[&str] = &[
    "Buuuh",
    "Você está assombrado",
    "Nossa que terminal vazio",
    "O que acontece se eu rodar shutdown?",
    "Tem alguém aí?",
    "Esse prompt está me olhando...",
    "Eu vi um processo zumbi, juro",
    "Você salvou o arquivo?",
];

#[repr(C)]
struct Winsize {
    row: u16,
    col: u16,
    xpixel: u16,
    ypixel: u16,
}

extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn tcgetpgrp(fd: c_int) -> c_int;
    fn getpgid(pid: c_int) -> c_int;
    fn getpgrp() -> c_int;
    fn getppid() -> c_int;
    fn fork() -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
    fn signal(signum: c_int, handler: usize) -> usize;
}

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: c_int) {
    STOP.store(true, Ordering::Relaxed);
}

// (linhas, colunas); None se falhar ou se o terminal for pequeno demais para o pet.
fn term_size(fd: c_int) -> Option<(usize, usize)> {
    let mut ws = Winsize { row: 0, col: 0, xpixel: 0, ypixel: 0 };
    let ok = unsafe { ioctl(fd, TIOCGWINSZ, &mut ws as *mut Winsize) } == 0;
    (ok && ws.row >= 3 && ws.col as usize > SPRITE_W + 1).then(|| (ws.row as usize, ws.col as usize))
}

struct Rng(u64);

impl Rng {
    fn seeded() -> Rng {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Rng(nanos | 1)
    }

    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
}

enum Mode {
    Walk,
    Idle,
    Blink,
    Say,
}

struct Pet {
    x: usize,
    dir: isize,
    mode: Mode,
    left: u32,
    text: &'static str,
}

impl Pet {
    fn new() -> Pet {
        Pet { x: 0, dir: 1, mode: Mode::Idle, left: 0, text: "" }
    }

    fn tick(&mut self, cols: usize, rng: &mut Rng) {
        let max_x = cols - SPRITE_W - 1;
        if self.left == 0 {
            self.text = "";
            match rng.next() % 20 {
                0..=7 => {
                    self.mode = Mode::Walk;
                    self.left = 10 + rng.next() % 30;
                    if rng.next() % 3 == 0 {
                        self.dir = -self.dir;
                    }
                }
                8..=13 => {
                    self.mode = Mode::Idle;
                    self.left = 5 + rng.next() % 15;
                }
                14..=16 => {
                    self.mode = Mode::Blink;
                    self.left = 2;
                }
                _ => {
                    let text = PHRASES[rng.next() as usize % PHRASES.len()];
                    if SPRITE_W + 1 + text.chars().count() < cols {
                        self.mode = Mode::Say;
                        self.left = 25;
                        self.text = text;
                    } else {
                        self.mode = Mode::Idle;
                        self.left = 5;
                    }
                }
            }
        }
        self.left -= 1;
        if let Mode::Walk = self.mode {
            let nx = self.x as isize + self.dir;
            if nx < 0 || nx > max_x as isize {
                self.dir = -self.dir;
            } else {
                self.x = nx as usize;
            }
        }
        // Após um resize a frase pode não caber mais: descarta e mantém tudo na tela.
        if !self.text.is_empty() && self.width() >= cols {
            self.text = "";
        }
        self.x = self.x.min(cols - 1 - self.width());
    }

    // Colunas ocupadas: sprite + " " + frase, se estiver falando.
    fn width(&self) -> usize {
        if self.text.is_empty() {
            SPRITE_W
        } else {
            SPRITE_W + 1 + self.text.chars().count()
        }
    }

    fn sprite(&self) -> &'static str {
        match self.mode {
            Mode::Blink => SPRITE_BLINK,
            _ => SPRITE,
        }
    }
}

// Reserva a última linha. IND + CUU garante que o cursor não fique na linha
// reservada (rola a tela só se ele estava nela) sem mexer na coluna.
fn setup(rows: usize, prev_rows: Option<usize>) -> String {
    let mut s = String::new();
    if let Some(p) = prev_rows {
        if p < rows {
            s.push_str(&format!("\x1b7\x1b[{};1H\x1b[2K\x1b8", p));
        }
    }
    s.push_str(&format!("\x1b7\x1b[r\x1b8\x1bD\x1b[A\x1b7\x1b[1;{}r\x1b8", rows - 1));
    s
}

// Reaplica a região (apps de tela cheia podem resetá-la) e desenha o pet.
fn frame(rows: usize, x: usize, sprite: &str, text: &str) -> String {
    let speech = if text.is_empty() { String::new() } else { format!(" {}", text) };
    format!(
        "\x1b7\x1b[1;{}r\x1b[{};1H\x1b[2K\x1b[{};{}H\x1b[0;96m{}\x1b[0m{}\x1b8",
        rows - 1,
        rows,
        rows,
        x + 1,
        sprite,
        speech
    )
}

fn restore(rows: usize) -> String {
    format!("\x1b7\x1b[r\x1b[{};1H\x1b[2K\x1b8", rows)
}

// PIDs dos outros processos chamados g-pet.
fn pet_pids() -> Vec<c_int> {
    let me = std::process::id() as c_int;
    let mut pids = Vec::new();
    if let Ok(dir) = std::fs::read_dir("/proc") {
        for e in dir.flatten() {
            let pid = match e.file_name().to_str().and_then(|s| s.parse::<c_int>().ok()) {
                Some(p) if p != me => p,
                _ => continue,
            };
            if std::fs::read_to_string(e.path().join("comm")).map_or(false, |c| c.trim() == "g-pet") {
                pids.push(pid);
            }
        }
    }
    pids
}

// (estado, tty_nr) de /proc/<pid>/stat. O comm vem entre parênteses e pode ter espaços.
fn stat_of(pid: c_int) -> Option<(char, i64)> {
    let s = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let mut f = s[s.rfind(')')? + 1..].split_whitespace();
    let state = f.next()?.chars().next()?;
    let tty_nr = f.nth(3)?.parse().ok()?; // pula ppid, pgrp, session
    Some((state, tty_nr))
}

// Manda SIGTERM a todos os outros g-pet (cada um restaura o terminal ao sair).
fn kill_all() -> usize {
    pet_pids().into_iter().filter(|&p| unsafe { kill(p, SIGTERM) } == 0).count()
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("kill") {
        match kill_all() {
            0 => println!("g-pet: nenhum g-pet rodando"),
            n => println!("g-pet: {} encerrado(s)", n),
        }
        return;
    }

    let mut tty = match OpenOptions::new().write(true).open("/dev/tty") {
        Ok(t) => t,
        Err(e) => {
            eprintln!("g-pet: não foi possível abrir /dev/tty: {}", e);
            std::process::exit(1);
        }
    };
    let fd = tty.as_raw_fd();

    // No máximo um g-pet por terminal (zumbis não contam).
    let my_tty = stat_of(std::process::id() as c_int).map_or(0, |(_, t)| t);
    if my_tty != 0 && pet_pids().into_iter().filter_map(stat_of).any(|(st, t)| st != 'Z' && t == my_tty) {
        eprintln!("g-pet: já existe um g-pet neste terminal (use `g-pet kill` para encerrar)");
        std::process::exit(1);
    }

    // Vai para background sozinho: o processo digitado volta ao shell na hora.
    let ppid = unsafe { getppid() };
    if unsafe { fork() } > 0 {
        std::process::exit(0);
    }

    unsafe {
        signal(SIGHUP, on_signal as *const () as usize);
        signal(SIGINT, on_signal as *const () as usize);
        signal(SIGTERM, on_signal as *const () as usize);
        signal(SIGTTOU, SIG_IGN);
    }
    let shell_pgid = unsafe { getpgid(ppid) };
    let own_pgid = unsafe { getpgrp() };
    // Shell no prompt (ou o próprio pet rodando em foreground).
    let shell_fg = || {
        let fg = unsafe { tcgetpgrp(fd) };
        fg == shell_pgid || fg == own_pgid
    };

    let mut rng = Rng::seeded();
    let mut pet = Pet::new();
    let mut rows = 0;
    let mut active = false;

    while !STOP.load(Ordering::Relaxed) {
        if unsafe { kill(ppid, 0) } != 0 {
            break;
        }
        match (shell_fg(), term_size(fd)) {
            (true, Some((r, c))) => {
                let mut out = String::new();
                if !active || r != rows {
                    out.push_str(&setup(r, if active { Some(rows) } else { None }));
                }
                active = true;
                rows = r;
                pet.tick(c, &mut rng);
                out.push_str(&frame(r, pet.x, pet.sprite(), pet.text));
                if tty.write_all(out.as_bytes()).is_err() {
                    break;
                }
            }
            _ => active = false,
        }
        sleep(TICK);
    }

    if rows > 0 && shell_fg() {
        let _ = tty.write_all(restore(rows).as_bytes());
    }
}
