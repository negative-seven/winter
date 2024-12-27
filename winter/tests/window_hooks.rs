#![allow(non_snake_case)]

use anyhow::Result;
use shared::input::MouseButton;
use std::time::Duration;
use test_utilities::{init_test, Architecture, Event, Instance};
use test_utilities_macros::test_for;

#[test_for(architecture, unicode)]
async fn RegisterClassEx(architecture: Architecture, unicode: bool) -> Result<()> {
    init_test();
    let stdout = Instance::new("hooks/misc/RegisterClassEx", architecture)
        .with_unicode_flag(unicode)
        .stdout_from_utf8_lossy()
        .await?;

    assert_eq!(stdout, "275\r\n");

    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct Message {
    milliseconds: u32,
    id: u32,
    w_parameter: usize,
    l_parameter: isize,
}

impl Message {
    fn new(milliseconds: u32, id: u32, w_parameter: usize, l_parameter: isize) -> Self {
        Self {
            milliseconds,
            id,
            w_parameter,
            l_parameter,
        }
    }
}

fn extract_messages_from_stdout(stdout: &[u8], message_ids: &[u32]) -> Vec<Message> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let mut tokens = line.split_ascii_whitespace();
            let milliseconds = tokens.next().unwrap().parse().unwrap();
            let id = tokens.next().unwrap().parse().unwrap();
            if !message_ids.contains(&id) {
                return None;
            }
            let w_parameter = tokens.next().unwrap().parse().unwrap();
            let l_parameter = tokens.next().unwrap().parse().unwrap();
            Some(Message {
                milliseconds,
                id,
                w_parameter,
                l_parameter,
            })
        })
        .collect()
}

#[test_for(architecture, unicode)]
async fn PeekMessage_with_key_messages(architecture: Architecture, unicode: bool) -> Result<()> {
    const WM_KEYDOWN: u32 = 256;
    const WM_KEYUP: u32 = 257;

    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let messages = extract_messages_from_stdout(
        &Instance::new("hooks/misc/PeekMessage", architecture)
            .with_unicode_flag(unicode)
            .with_events([
                key_event(65, true),
                key_event(65, true),
                key_event(66, true),
                key_event(67, true),
                Event::AdvanceTime(Duration::from_millis(77)),
                key_event(65, true),
                key_event(67, true),
                Event::AdvanceTime(Duration::from_millis(18)),
                key_event(68, true),
                key_event(67, false),
                key_event(67, false),
                Event::AdvanceTime(Duration::from_millis(1)),
                key_event(37, true),
                key_event(65, false),
                key_event(37, false),
                key_event(66, false),
                key_event(68, false),
                Event::AdvanceTime(Duration::from_millis(1)),
                key_event(40, false),
                key_event(40, true),
                Event::AdvanceTime(Duration::from_millis(3)),
            ])
            .stdout()
            .await?,
        &[WM_KEYDOWN, WM_KEYUP],
    );

    let message = Message::new;
    assert_eq!(
        messages,
        [
            message(1, WM_KEYDOWN, 65, 1),
            message(1, WM_KEYDOWN, 65, (1 << 30) | 1),
            message(1, WM_KEYDOWN, 66, 1),
            message(1, WM_KEYDOWN, 67, 1),
            message(78, WM_KEYDOWN, 65, (1 << 30) | 1),
            message(78, WM_KEYDOWN, 67, (1 << 30) | 1),
            message(96, WM_KEYDOWN, 68, 1),
            message(96, WM_KEYUP, 67, (1 << 31) | (1 << 30) | 1),
            message(96, WM_KEYUP, 67, (1 << 31) | 1),
            message(97, WM_KEYDOWN, 37, 1),
            message(97, WM_KEYUP, 65, (1 << 31) | (1 << 30) | 1),
            message(97, WM_KEYUP, 37, (1 << 31) | (1 << 30) | 1),
            message(97, WM_KEYUP, 66, (1 << 31) | (1 << 30) | 1),
            message(97, WM_KEYUP, 68, (1 << 31) | (1 << 30) | 1),
            message(98, WM_KEYUP, 40, (1 << 31) | 1),
            message(98, WM_KEYDOWN, 40, 1),
        ]
    );
    Ok(())
}

#[test_for(architecture, unicode)]
async fn GetMessage_with_key_messages(architecture: Architecture, unicode: bool) -> Result<()> {
    const WM_KEYDOWN: u32 = 256;
    const WM_KEYUP: u32 = 257;

    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }

    init_test();
    let messages = extract_messages_from_stdout(
        &Instance::new("hooks/misc/GetMessage", architecture)
            .with_unicode_flag(unicode)
            .with_events([
                key_event(65, true),
                key_event(65, true),
                key_event(66, true),
                key_event(67, true),
                Event::AdvanceTime(Duration::from_millis(12)),
                key_event(65, true),
                key_event(67, true),
                Event::AdvanceTime(Duration::from_millis(34)),
                key_event(68, true),
                key_event(67, false),
                key_event(67, false),
                Event::AdvanceTime(Duration::from_millis(56)),
                key_event(37, true),
                key_event(65, false),
                key_event(37, false),
                key_event(66, false),
                key_event(68, false),
                Event::AdvanceTime(Duration::from_millis(78)),
                key_event(40, false),
                key_event(40, true),
                Event::AdvanceTime(Duration::from_millis(90)),
            ])
            .stdout()
            .await?,
        &[WM_KEYDOWN, WM_KEYUP],
    );

    let message = Message::new;
    assert_eq!(
        messages,
        [
            message(12, WM_KEYDOWN, 65, 1),
            message(12, WM_KEYDOWN, 65, (1 << 30) | 1),
            message(12, WM_KEYDOWN, 66, 1),
            message(12, WM_KEYDOWN, 67, 1),
            message(46, WM_KEYDOWN, 65, (1 << 30) | 1),
            message(46, WM_KEYDOWN, 67, (1 << 30) | 1),
            message(102, WM_KEYDOWN, 68, 1),
            message(102, WM_KEYUP, 67, (1 << 31) | (1 << 30) | 1),
            message(102, WM_KEYUP, 67, (1 << 31) | 1),
            message(180, WM_KEYDOWN, 37, 1),
            message(180, WM_KEYUP, 65, (1 << 31) | (1 << 30) | 1),
            message(180, WM_KEYUP, 37, (1 << 31) | (1 << 30) | 1),
            message(180, WM_KEYUP, 66, (1 << 31) | (1 << 30) | 1),
            message(180, WM_KEYUP, 68, (1 << 31) | (1 << 30) | 1),
            message(270, WM_KEYUP, 40, (1 << 31) | 1),
            message(270, WM_KEYDOWN, 40, 1),
        ]
    );
    Ok(())
}

#[test_for(architecture, unicode)]
async fn PeekMessage_with_mouse_messages(architecture: Architecture, unicode: bool) -> Result<()> {
    const WM_MOUSEMOVE: u32 = 512;
    const WM_LBUTTONDOWN: u32 = 513;
    const WM_LBUTTONUP: u32 = 514;
    const WM_RBUTTONDOWN: u32 = 516;
    const WM_RBUTTONUP: u32 = 517;
    const WM_MBUTTONDOWN: u32 = 519;
    const WM_MBUTTONUP: u32 = 520;
    const WM_XBUTTONDOWN: u32 = 523;
    const WM_XBUTTONUP: u32 = 524;

    fn key_event(id: u8, state: bool) -> Event {
        Event::SetKeyState { id, state }
    }
    fn button_event(button: MouseButton, state: bool) -> Event {
        Event::SetMouseButtonState { button, state }
    }

    init_test();
    let messages = extract_messages_from_stdout(
        &Instance::new("hooks/misc/PeekMessage", architecture)
            .with_unicode_flag(unicode)
            .with_events([
                button_event(MouseButton::X1, true),
                button_event(MouseButton::Left, true),
                button_event(MouseButton::Middle, true),
                button_event(MouseButton::X2, true),
                button_event(MouseButton::Right, true),
                button_event(MouseButton::Middle, false),
                button_event(MouseButton::X2, false),
                button_event(MouseButton::X1, false),
                button_event(MouseButton::Right, false),
                button_event(MouseButton::Left, false),
                Event::SetMousePosition { x: 111, y: 222 },
                button_event(MouseButton::Right, true),
                Event::SetMousePosition { x: 44, y: 33 },
                button_event(MouseButton::X1, true),
                key_event(162, true),
                button_event(MouseButton::X1, false),
                key_event(161, true),
                button_event(MouseButton::Right, false),
                key_event(162, false),
                button_event(MouseButton::X2, true),
                key_event(161, false),
                button_event(MouseButton::X2, false),
                Event::AdvanceTime(Duration::from_millis(100)),
            ])
            .stdout()
            .await?,
        &[
            WM_MOUSEMOVE,
            WM_LBUTTONDOWN,
            WM_LBUTTONUP,
            WM_RBUTTONDOWN,
            WM_RBUTTONUP,
            WM_MBUTTONDOWN,
            WM_MBUTTONUP,
            WM_XBUTTONDOWN,
            WM_XBUTTONUP,
        ],
    );

    let message = |a, b, c| Message::new(1, a, b, c);
    assert_eq!(
        messages,
        [
            message(WM_XBUTTONDOWN, (1 << 16) | 0x20, 0),
            message(WM_LBUTTONDOWN, 0x21, 0),
            message(WM_MBUTTONDOWN, 0x31, 0),
            message(WM_XBUTTONDOWN, (2 << 16) | 0x71, 0),
            message(WM_RBUTTONDOWN, 0x73, 0),
            message(WM_MBUTTONUP, 0x63, 0),
            message(WM_XBUTTONUP, (2 << 16) | 0x23, 0),
            message(WM_XBUTTONUP, (1 << 16) | 0x3, 0),
            message(WM_RBUTTONUP, 0x1, 0),
            message(WM_LBUTTONUP, 0x0, 0),
            message(WM_MOUSEMOVE, 0x0, (222 << 16) | 111),
            message(WM_RBUTTONDOWN, 0x2, (222 << 16) | 111),
            message(WM_MOUSEMOVE, 0x2, (33 << 16) | 44),
            message(WM_XBUTTONDOWN, (1 << 16) | 0x22, (33 << 16) | 44),
            message(WM_XBUTTONUP, (1 << 16) | 0xa, (33 << 16) | 44),
            message(WM_RBUTTONUP, 0xc, (33 << 16) | 44),
            message(WM_XBUTTONDOWN, (2 << 16) | 0x44, (33 << 16) | 44),
            message(WM_XBUTTONUP, 2 << 16, (33 << 16) | 44),
        ]
    );
    Ok(())
}
