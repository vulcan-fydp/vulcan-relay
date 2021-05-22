use vulcan_relay::signal_schema::SignalSchema;

fn main() {
    println!("{}", &SignalSchema::default().sdl());
}
