use vulcan_relay::control_schema::ControlSchema;

fn main() {
    println!("{}", &ControlSchema::default().sdl());
}
