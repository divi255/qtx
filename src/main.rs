use cpp_core::{Ptr, StaticUpcast};
use qt_core::{qs, slot, QBox, QObject, QPtr, SignalNoArgs, SlotNoArgs};
use qt_ui_tools::ui_form;
use qt_widgets::{QApplication, QLineEdit, QPushButton, QWidget};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use unsafe_send_sync::UnsafeSend;

// commands to the background worker
enum Command {
    Reset,
    Quit,
}

// data from the background worker (alternative: slots, but objects must be Qt-ized)
enum Data {
    Counter(u64),
}

// background worker
fn worker(
    command_rx: mpsc::Receiver<Command>,
    data_tx: mpsc::SyncSender<Data>,
    data_signal: UnsafeSend<QBox<SignalNoArgs>>,
) {
    let mut counter = 0;
    loop {
        while let Ok(command) = command_rx.try_recv() {
            match command {
                Command::Reset => counter = 0,
                Command::Quit => break,
            }
        }
        if data_tx.send(Data::Counter(counter)).is_ok() {
            unsafe {
                println!("emitting signal from {:?}", thread::current().id());
                data_signal.emit();
            }
        }
        thread::sleep(Duration::from_secs(1));
        counter += 1;
    }
}

// main window
#[ui_form("../ui/main.ui")]
struct Main {
    widget: QBox<QWidget>,
    counter: QPtr<QLineEdit>,
    btn_reset: QPtr<QPushButton>,
}

// UI
struct Ui {
    window: Main,
    command_tx: mpsc::SyncSender<Command>,
    data_rx: mpsc::Receiver<Data>,
}

// reqired to transform Rust functions into slots
impl StaticUpcast<QObject> for Ui {
    unsafe fn static_upcast(ptr: Ptr<Self>) -> Ptr<QObject> {
        ptr.window.widget.as_ptr().static_upcast()
    }
}

impl Ui {
    // Rc required to transform Rust functions into slots
    fn new(command_tx: mpsc::SyncSender<Command>, data_rx: mpsc::Receiver<Data>) -> Rc<Self> {
        unsafe {
            let window = Main::load();
            let ui = Rc::new(Ui {
                window,
                command_tx,
                data_rx,
            });
            ui.window
                .btn_reset
                .clicked()
                .connect(&ui.slot_handle_btn_reset());
            //let ctx = ui.command_tx.clone();
            //ui.window
            //.btn_reset
            //.clicked()
            //.connect(&SlotNoArgs::new(&ui.window.widget, move || {
            //let _ = ctx.send(Command::Reset);
            //}));
            ui
        }
    }
    unsafe fn show(self: &Rc<Self>) {
        println!("running show in {:?}", thread::current().id());
        self.window.widget.show();
    }
    #[slot(SlotNoArgs)]
    unsafe fn handle_btn_reset(self: &Rc<Self>) {
        let _ = self.command_tx.send(Command::Reset);
    }
    #[slot(SlotNoArgs)]
    unsafe fn handle_data(self: &Rc<Self>) {
        println!("running handle data in {:?}", thread::current().id());
        while let Ok(data) = self.data_rx.try_recv() {
            match data {
                Data::Counter(v) => {
                    self.window.counter.set_text(&qs(v.to_string()));
                }
            }
        }
    }
}

fn main() {
    // 4K hack
    std::env::set_var("QT_AUTO_SCREEN_SCALE_FACTOR", "1");
    QApplication::init(|_| {
        // command channel
        let (command_tx, command_rx) = mpsc::sync_channel::<Command>(64);
        // data channel
        let (data_tx, data_rx) = mpsc::sync_channel::<Data>(64);
        // construct UI
        let ui = Ui::new(command_tx.clone(), data_rx);
        unsafe {
            // data signal
            let data_signal = UnsafeSend::new(SignalNoArgs::new());
            // connect data signal with UI handle_data slot method
            data_signal.connect(&ui.slot_handle_data());
            // run the background worker
            thread::spawn(move || {
                worker(command_rx, data_tx, data_signal);
            });
            // display the UI
            ui.show();
            // exec the Qt application
            let result: i32 = QApplication::exec();
            // optionally terminate the background worker
            command_tx.send(Command::Quit).unwrap();
            result
        }
    })
}
