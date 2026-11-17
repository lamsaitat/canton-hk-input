use canton_core::{Engine, EngineAction, EngineConfig, KeyEvent, Lexicon};
use std::io::{self, BufRead, Write};

fn main() {
    println!("CantonHK (廣東拼音) CLI Simulator");
    println!("Loaded {} keys from cantonhk.cin.", Lexicon::key_count());
    println!("Commands: type keys directly (e.g. 'ngo ', 'heung1gong2'), ':toggle' for 中/英, ':quit' to exit.\n");

    let mut engine = Engine::new(EngineConfig::default());
    let mut output = String::new();
    let stdin = io::stdin();

    loop {
        print!(
            "[{}] Output=\"{}\" Preedit=\"{}\"> ",
            if engine.is_chinese_mode() { "中" } else { "英" },
            output,
            engine.preedit()
        );
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap() == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        if trimmed == ":quit" || trimmed == ":q" {
            break;
        }
        if trimmed == ":toggle" {
            engine.toggle_chinese_mode();
            continue;
        }
        if trimmed == ":clear" {
            output.clear();
            engine.reset();
            continue;
        }

        for ch in trimmed.chars() {
            let ev = match ch {
                ' ' => KeyEvent::Space,
                '>' => KeyEvent::PageDown,
                '<' => KeyEvent::PageUp,
                '\\' => KeyEvent::Backspace,
                _ => KeyEvent::Char(ch),
            };
            match engine.handle_key(ev) {
                EngineAction::PassThrough => output.push(ch),
                EngineAction::Commit(s) => output.push_str(&s),
                EngineAction::CommitAndUpdate {
                    committed,
                    next_page,
                } => {
                    output.push_str(&committed);
                    print_page(&next_page);
                }
                EngineAction::UpdateUi(page) => print_page(&page),
                EngineAction::Cancel => println!("  [Cancelled]"),
                EngineAction::Reject => println!("  [Rejected '{}']", ch),
            }
        }
        println!();
    }
}

fn print_page(page: &canton_core::CandidatePage) {
    print!(
        "  [{}] ({}/{}) ",
        page.preedit,
        page.page_index + 1,
        page.total_pages
    );
    for (idx, item) in page.items.iter().enumerate() {
        let label = if idx == 9 { 0 } else { idx + 1 };
        if idx == page.selected_in_page {
            print!("[{}.{}] ", label, item);
        } else {
            print!("{}.{} ", label, item);
        }
    }
    println!();
}
