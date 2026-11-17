//! `freenapse.exe`: launched without arguments it is the tray app (no console
//! window); with a command it is the CLI, printing to the console it was
//! started from.

#![windows_subsystem = "windows"]
#![allow(clippy::collapsible_if, clippy::collapsible_match)]

use std::process::ExitCode;

use freenapse::config::{Config, describe};
use freenapse::device::Mouse;
use freenapse::protocol::{Report, TID_MOUSE};
use freenapse::sensor::Tracking;
use freenapse::{apply, config_path};

const USAGE: &str = "freenapse - 轻量级雷蛇鼠标控制工具（替代雷云）

只向主人验证过的鼠标写入，其它雷蛇鼠标一律只读：`freenapse status` 会说明当前连的是哪一只，
设置窗口的“设备”页会把它们全列出来。

无线鼠标只在醒着的时候应答：执行命令时请让它一直有移动。

用法：
  freenapse status                       电池、DPI、回报率、休眠、传感器、接收器
  freenapse probe [--serials]            鼠标原始应答，用于给新鼠标开 issue（默认遮蔽序列号，加 --serials 显示）
  freenapse apply                        把配置文件写入鼠标（板载存储）
  freenapse config                       打印配置文件路径（不存在时按鼠标当前设置创建）
  freenapse import                       用鼠标当前的设置覆盖配置文件
  freenapse dpi <x> [y]                  设置即时 DPI（100 到鼠标上限）
  freenapse stages <active> <dpi>...     设置最多 5 个 DPI 档位，例如 stages 2 400 800 1600
  freenapse polling <hz>                 125|250|500|1000|2000|4000|8000
  freenapse idle <seconds>               空闲 N 秒后休眠（60..900）
  freenapse low-battery <percent>        低功耗模式门槛（5..100，仅在 <= 1000 Hz 时生效）
  freenapse lift smart <mm>              智能追踪，1..3 毫米
  freenapse lift asym <liftoff> <landing>    非对称抬升：抬起 2..26，落下 1..25（雷云默认 13 12）
  freenapse rotation <degrees>           传感器旋转 -30..30，0 = 关闭
  freenapse lighting [<zone> <effect>]   查看或设置某个区域的灯光：off、spectrum、static:#rrggbb、
                                         breathing:#rrggbb、breathing:random（区域：logo、wheel）
  freenapse lighting brightness <0..100> 所有区域的亮度
  freenapse dynamic off|classic|natural|jump|custom   动态灵敏度（custom 用配置文件里的曲线）
  freenapse dongle-led <1|2|3>           HyperPolling 接收器指示灯：电量 | 回报率 | 关闭
  freenapse buttons [profile]            显示板载按键映射（默认配置 1）
  freenapse profiles                     板载配置：已存几个、鼠标最多能存几个
  freenapse profile <1..5>               选择板载配置（鼠标还有空位时会新建）
  freenapse macros                       鼠标里存的宏，以及剩余的宏存储
  freenapse macro-read <id>              以配置文件的写法打印存好的宏
  freenapse macro-write <id>             把配置文件里这个 id 的宏存进鼠标
  freenapse macro-delete <id>            从鼠标里删掉一个宏
  freenapse pair-status                  接收器已配对的设备、自动配对状态
  freenapse pair [pid]                   让接收器进入配对模式（默认用这只鼠标的无线 ID）
  freenapse pair-cancel [pid]            退出配对模式
  freenapse unpair --yes [pid]           忘掉配对（鼠标会停止工作，直到重新配对）
  freenapse games                        在 Steam、Epic、GOG、EA、Battle.net、Riot、育碧、Xbox 里找到的游戏
  freenapse autostart on|off             开机时启动托盘程序
  freenapse ui                           打开设置窗口
  freenapse raw <class> <id> <size> [args..]   发送原始命令（十六进制字节）
  freenapse sensor-dump                  传感器原始寄存器
  freenapse watch [n]                    每秒读一次电量
  freenapse version                      打印版本

不带参数运行 freenapse.exe 就是托盘图标（电池、DPI 档位与回报率切换、
游戏配置、宏播放）。

任何命令和托盘都接受 --log 或 --log=<文件>，记录与鼠标的每一次交互
（默认写在配置文件旁边的 hid.log）。崩溃会写 crash.txt，两个都请附在 issue 里。
";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // --log[=file]: HID traffic log, in any mode.
    if let Some(i) = args.iter().position(|a| a == "--log" || a.starts_with("--log=")) {
        let flag = args.remove(i);
        let path = match flag.strip_prefix("--log=") {
            Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
            _ => freenapse::diag::default_hid_log_path(),
        };
        freenapse::diag::set_hid_log(path);
    }
    let first = args.first().map(String::as_str);
    let gui = first.is_none() || first == Some("--tray") || first == Some("--settings");
    freenapse::diag::install_panic_hook(gui);
    if gui {
        // Tray mode: no console. A second instance opens the settings window of the running one.
        freenapse::tray::run(args.iter().any(|a| a == "--settings"));
        return ExitCode::SUCCESS;
    }
    // CLI mode: this is a windowed executable, so attach to the console we were started from.
    unsafe {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match run(cmd, &args[1..]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误：{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: &str, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "config" => {
            let opened = Mouse::open(); // picks the mouse's own config file
            let p = config_path();
            if !p.exists() {
                match opened.and_then(|m| apply::import(&m)) {
                    Ok(cfg) => {
                        Config::write_new(&p, &cfg)?;
                        println!("已按鼠标当前的设置创建");
                    }
                    Err(e) => {
                        Config::write_new(&p, &Config::default())?;
                        println!("已用默认值创建（{e}）");
                    }
                }
            }
            println!("{}", p.display());
            return Ok(());
        }
        "ui" => {
            // The window lives in the tray process: start one (or ask the running one) with --settings.
            let exe = std::env::current_exe()?;
            std::process::Command::new(&exe).arg("--settings").spawn().map_err(|e| format!("无法启动 {}：{e}", exe.display()))?;
            return Ok(());
        }
        "autostart" => {
            let on = match args.first().map(String::as_str) {
                Some("on") => true,
                Some("off") => false,
                _ => return Err("autostart on|off".into()),
            };
            freenapse::autostart::set(on)?;
            println!("autostart {}", if on { "enabled" } else { "disabled" });
            return Ok(());
        }
        "games" => {
            // Installed games found in the launchers, and which ones have a profile.
            let cfg = Config::load(&config_path()).unwrap_or_default();
            let games = freenapse::library::scan();
            for g in &games {
                let profile = cfg.games.iter().find(|p| p.matches(&format!("{}\\{}", g.dir, if g.exe.is_empty() { "?" } else { &g.exe })));
                let mark = profile.map(|p| format!("  [profile: {}]", p.label())).unwrap_or_default();
                println!("{:<16} {}{mark}\n                 {}", g.source, g.name, g.dir);
            }
            println!("{} 个游戏", games.len());
            return Ok(());
        }
        "help" | "-h" | "--help" => {
            print!("{}", USAGE.replacen("freenapse -", &format!("freenapse {} -", freenapse::VERSION), 1));
            return Ok(());
        }
        "version" | "-V" | "--version" => {
            println!("freenapse {}", freenapse::VERSION);
            return Ok(());
        }
        _ => {}
    }

    let mouse = Mouse::open()?;
    match cmd {
        "status" => status(&mouse)?,
        "import" => {
            let mut cfg = apply::import(&mouse)?;
            let p = config_path();
            cfg.preserve_device_access(&p);
            Config::write_new(&p, &cfg)?;
            println!("已写入 {}", p.display());
            print!("{}", cfg.render());
        }
        "apply" => {
            let cfg = Config::load_or_create(&config_path())?;
            let changes = apply::apply(&mouse, &cfg, cfg.polling.rate)?;
            if changes.is_empty() {
                println!("鼠标已经和配置一致");
            } else {
                for c in changes {
                    println!("{c}");
                }
            }
        }
        "dpi" => {
            let x: u16 = arg(args, 0, "dpi x")?;
            let y: u16 = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(x);
            mouse.set_dpi(x, y)?;
            println!("DPI 已设为 {:?}", mouse.dpi()?);
        }
        "stages" => {
            let active: u8 = arg(args, 0, "当前档位")?;
            let stages: Vec<(u16, u16)> = args[1..].iter().map(|s| s.parse::<u16>().map(|d| (d, d))).collect::<Result<_, _>>()?;
            if stages.is_empty() || stages.len() > 5 {
                return Err("请给 1 到 5 个 DPI 值".into());
            }
            mouse.set_dpi_stages(active, &stages)?;
            println!("{:?}", mouse.dpi_stages()?);
        }
        "polling" => {
            let hz: u16 = arg(args, 0, "回报率")?;
            mouse.set_polling_rate(hz)?;
            println!("回报率 {} Hz", mouse.polling_rate()?);
        }
        "idle" => {
            let s: u16 = arg(args, 0, "seconds")?;
            mouse.set_idle_time(s)?;
            println!("空闲超时 {} 秒", mouse.idle_time()?);
        }
        "low-battery" => {
            let p: u8 = arg(args, 0, "percent")?;
            mouse.set_low_battery_threshold(p)?;
            println!("低电量门槛 {}%", mouse.low_battery_threshold()?);
        }
        "lift" => {
            let kind = args.first().map(String::as_str).ok_or("缺少模式")?;
            let v1: u8 = arg(args, 1, "value")?;
            let v2: u8 = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(1);
            let t = match kind {
                "smart" => Tracking::Smart { mm: v1 },
                "asym" | "smart-asym" => Tracking::Asymmetric { lift: v1, land: v2 },
                _ => return Err("抬升模式必须是 smart <毫米> 或 asym <抬起> <落下>".into()),
            };
            let plan = t.plan()?;
            mouse.apply_tracking(&plan)?;
            println!("传感器：{}", mouse.sensor()?.describe());
        }
        "lighting" => {
            let zones = mouse.spec().lighting;
            if zones.is_empty() {
                return Err("这只鼠标没有灯光".into());
            }
            match (args.first(), args.get(1)) {
                (Some(z), Some(v)) if z == "brightness" => {
                    let b: u8 = v.parse().map_err(|_| format!("亮度 “{v}” 无效"))?;
                    for (zone, _) in zones {
                        mouse.set_brightness(zone, b.min(100))?;
                    }
                }
                (Some(z), Some(v)) => mouse.set_lighting(z, freenapse::lighting::Effect::parse(v)?)?,
                (None, _) => {}
                _ => return Err("lighting [<区域> <灯效> | brightness <0..100>]".into()),
            }
            for (zone, _) in zones {
                let e = mouse.lighting(zone)?.map(|e| e.spec()).unwrap_or_else(|| "（其它）".into());
                println!("{zone:<6} {e}，亮度 {}%", mouse.brightness(zone)?);
            }
        }
        "rotation" => {
            let deg: i8 = arg(args, 0, "degrees")?;
            if !(-30..=30).contains(&deg) {
                return Err("旋转必须在 -30..30 之间".into());
            }
            mouse.set_rotation(deg)?;
            let (on, d) = mouse.rotation()?;
            println!("旋转 {}", if on { format!("{d} 度") } else { "off".into() });
        }
        "profiles" => {
            let (n, max) = mouse.profiles()?;
            println!("板载配置：已存 {n} 个，鼠标最多能存 {max} 个：id 为 {:?}", mouse.profile_list()?);
            match mouse.active_profile() {
                Ok(a) => println!("当前：{a}"),
                Err(e) => println!("当前：这只鼠标不上报（{e}）；只有一个配置时就是 1"),
            }
        }
        "profile" => {
            let id: u8 = arg(args, 0, "配置 id")?;
            mouse.select_profile(id).map_err(|e| {
                let (n, max) = mouse.profiles().unwrap_or((1, 1));
                format!("{e}（鼠标里存了 {n} 个，最多 {max} 个）")
            })?;
            println!("已选择板载配置 {id}；在配置里写 profile.active = {id} 可以一直用它");
        }
        "macros" => {
            let st = mouse.macro_storage()?;
            println!("宏存储：可用 {} / {} 字节（{} 字节可回收），id 最大到 {}", st.available, st.total, st.recyclable, st.max_id);
            let ids = mouse.macro_ids()?;
            if ids.is_empty() {
                println!("没有存宏");
            }
            for id in ids {
                let name = mouse.macro_name(id).unwrap_or_default();
                let size = mouse.macro_data_size(id).map(|s| s.to_string()).unwrap_or_else(|e| e.to_string());
                println!("{id:>5}  {size:>6} 字节  {name}");
            }
        }
        "macro-read" => {
            let id: u16 = arg(args, 0, "宏 id")?;
            let data = mouse.macro_data(id)?;
            let events = freenapse::macros::decode(&data);
            println!("name = {:?}", mouse.macro_name(id).unwrap_or_default());
            println!("events = {:?}", freenapse::macros::event_specs(&events));
            println!("bytes = {data:02x?}");
        }
        "macro-write" => {
            let id: u16 = arg(args, 0, "宏 id")?;
            let cfg = Config::load_or_create(&config_path())?;
            let m = cfg.macro_by_id(id).ok_or_else(|| format!("宏 {id} 不在配置文件里"))?;
            let bytes = m.bytes();
            mouse.write_macro(id, &m.name, &bytes)?;
            let back = mouse.macro_data(id)?;
            println!("宏 {id}（{}）已存入：{} 字节，回读结果 {}", m.name, bytes.len(), if back == bytes { "一致" } else { "不一致" });
        }
        "macro-delete" => {
            let id: u16 = arg(args, 0, "宏 id")?;
            mouse.delete_macro(id)?;
            println!("宏 {id} 已删除；剩余 id 为 {:?}", mouse.macro_ids()?);
        }
        "pair-status" => {
            for d in mouse.paired_devices()? {
                println!("已配对：产品 ID 0x{:04x}，连接状态 {}", d.product_id, d.status);
            }
            match mouse.auto_pairing() {
                Ok(on) => println!("自动配对：{}", if on { "on" } else { "off" }),
                Err(e) => println!("自动配对：{e}"),
            }
        }
        "pair" | "pair-cancel" | "unpair" => {
            let yes = args.iter().any(|a| a == "--yes");
            let pid = args
                .iter()
                .find(|a| !a.starts_with("--"))
                .map(|s| u16::from_str_radix(s.trim_start_matches("0x"), 16))
                .transpose()?
                .unwrap_or(mouse.spec().product_id);
            match cmd {
                "pair" => {
                    let mode = mouse.start_pairing(pid)?;
                    println!("接收器已进入配对模式，产品 ID 0x{pid:04x}（应答 {mode}）；把鼠标关掉再打开，靠近接收器");
                }
                "pair-cancel" => {
                    mouse.cancel_pairing(pid)?;
                    println!("已取消配对");
                }
                _ => {
                    if !yes {
                        return Err("unpair 会把鼠标从接收器里删掉；加 --yes 确认".into());
                    }
                    mouse.unpair(pid)?;
                    println!("产品 ID 0x{pid:04x} 已解除配对；运行 `freenapse pair` 可以重新配对");
                }
            }
        }
        "dynamic" => {
            let what = args.first().map(String::as_str).ok_or("dynamic off|classic|natural|jump|custom")?;
            if what == "off" {
                mouse.set_acceleration(false, freenapse::accel::Mode::Classic, None)?;
            } else {
                let mode = freenapse::accel::Mode::from_name(what).ok_or("dynamic off|classic|natural|jump|custom")?;
                let cfg = Config::load_or_create(&config_path())?;
                let samples = cfg.acceleration.samples();
                mouse.set_acceleration(true, mode, Some(&samples))?;
            }
            let (on, mode) = mouse.acceleration()?;
            println!("动态灵敏度：{}", if on { mode.name() } else { "off" });
        }
        "dongle-led" => {
            let m: u8 = arg(args, 0, "mode")?;
            mouse.set_dongle_led_mode(m)?;
            println!("接收器指示灯模式 {}", mouse.dongle_led_mode()?);
        }
        "buttons" => {
            let profile: u8 = args.first().map(|s| s.parse()).transpose()?.unwrap_or(1);
            for b in mouse.spec().buttons {
                let name = b.name;
                for hs in [false, true] {
                    let layer = if hs { "hypershift" } else { "" };
                    match mouse.button(profile, b.code, hs) {
                        Ok(f) => println!("{name:<12}{layer:<12}{}", describe(&f)),
                        Err(e) => println!("{name:<12}{layer:<12}出错：{e}"),
                    }
                }
            }
        }
        "raw" => {
            let (tid, args) = match args.first().map(String::as_str) {
                Some("-d") | Some("--dongle") => (freenapse::protocol::TID_DONGLE, &args[1..]),
                _ => (TID_MOUSE, args),
            };
            let bytes: Vec<u8> = args.iter().map(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16)).collect::<Result<_, _>>()?;
            if bytes.len() < 3 {
                return Err("raw [-d] <类别> <id> <长度> [参数..]".into());
            }
            let req = Report::new(tid, bytes[0], bytes[1], bytes[2]).with_args(&bytes[3..]);
            let r = mouse.raw(&req)?;
            let n = (r.data_size as usize).clamp(8, 80);
            println!("status={:02x} size={} args={:02x?}", r.status, r.data_size, &r.arguments[..n]);
        }
        "sensor-dump" => {
            for (name, v) in mouse.sensor_dump() {
                match v {
                    Ok(b) => println!("{name}：{b:02x?}"),
                    Err(e) => println!("{name}：出错 {e}"),
                }
            }
        }
        "probe" => {
            let show_serials = args.iter().any(|a| a == "--serials");
            eprintln!("正在探测：大约半分钟，请让鼠标保持唤醒（时不时动一下）");
            // Line by line as the answers come in, so a stalled probe shows where it stands.
            freenapse::probe::run_with(&mouse, show_serials, &mut |line| {
                use std::io::Write;
                println!("{line}");
                let _ = std::io::stdout().flush();
            });
        }
        "watch" => {
            let n: u32 = args.first().map(|s| s.parse()).transpose()?.unwrap_or(30);
            for _ in 0..n {
                let t = std::time::Instant::now();
                let r = mouse.battery();
                println!("{:?} in {:?}  (user idle {} ms)", r.map(|b| b.percent), t.elapsed(), freenapse::idle_ms());
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
        _ => {
            eprint!("{USAGE}");
            return Err(format!("未知命令 {cmd}").into());
        }
    }
    Ok(())
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize, what: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    let s = args.get(i).ok_or_else(|| format!("缺少 {what}"))?;
    s.parse().map_err(|e| format!("{what} “{s}” 无效：{e}"))
}

fn status(mouse: &Mouse) -> Result<(), Box<dyn std::error::Error>> {
    let spec = mouse.spec();
    let trust = match (spec.verified, mouse.writes_allowed()) {
        (true, _) => "",
        (false, true) => "  [未验证；已由 [device] allow_unverified 允许写入]",
        (false, false) => "  [未验证：只读，直到 [device] allow_unverified 里的名单包含它]",
    };
    println!("设备:         {} （产品 ID 0x{:04x}）{trust}", spec.name, spec.product_id);
    println!("连接方式:     {}", if mouse.is_wireless() { "无线（通过接收器）" } else { "有线" });
    if mouse.is_wireless()
        && let Ok((a, b)) = mouse.dongle_firmware()
    {
        println!("接收器:       固件 {a}.{b}  序列号 {}", mouse.dongle_serial().unwrap_or_default());
    }
    match mouse.firmware() {
        Ok((a, b)) => println!("鼠标:         固件 {a}.{b}  序列号 {}", mouse.serial().unwrap_or_default()),
        Err(e) => {
            println!("鼠标:         {e}");
            return Ok(());
        }
    }
    let b = mouse.battery()?;
    if b.wired {
        println!("电池:         无（有线）");
    } else {
        println!("电池:         {}%{}", b.percent, if b.charging { "（充电中）" } else { "" });
    }
    println!("DPI:          {:?}", mouse.dpi()?);
    let st = mouse.dpi_stages()?;
    println!("DPI 档位:     当前第 {} 档，共 {:?}", st.active, st.stages.iter().map(|s| s.0).collect::<Vec<_>>());
    println!("回报率:       {}", mouse.polling_rate().map(|p| format!("{p} Hz")).unwrap_or_else(|e| e.to_string()));
    if spec.battery {
        println!("空闲休眠:     {}", mouse.idle_time().map(|p| format!("{p} 秒")).unwrap_or_else(|e| e.to_string()));
        println!("低电量:       {}", mouse.low_battery_threshold().map(|p| format!("{p}%")).unwrap_or_else(|e| e.to_string()));
    }
    match mouse.sensor() {
        Ok(s) => println!("传感器:       {}", s.describe()),
        Err(e) => println!("传感器:       {e}"),
    }
    match mouse.rotation() {
        Ok((true, d)) => println!("旋转:         {d} 度"),
        Ok((false, _)) => println!("旋转:         关闭"),
        Err(e) => println!("旋转:         {e}"),
    }
    match mouse.acceleration() {
        Ok((true, m)) => println!("动态灵敏度:   {}", m.name()),
        Ok((false, _)) => println!("动态灵敏度:   关闭"),
        Err(e) => println!("动态灵敏度:   {e}"),
    }
    for (zone, _) in spec.lighting {
        let effect = match mouse.lighting(zone) {
            Ok(Some(e)) => e.spec(),
            Ok(None) => "freenapse 不写的灯效".into(),
            Err(e) => e.to_string(),
        };
        let b = mouse.brightness(zone).map(|b| format!("，亮度 {b}%")).unwrap_or_default();
        println!("灯光:         {zone} {effect}{b}");
    }
    if let Ok((n, max)) = mouse.profiles() {
        println!("配置:         板载 {n} 个（鼠标最多 {max} 个），正在用配置 {}", mouse.profile());
    }
    if spec.pairing {
        println!("接收器灯:     {}", mouse.dongle_led_mode().map(|p| p.to_string()).unwrap_or_else(|e| e.to_string()));
    }
    Ok(())
}
