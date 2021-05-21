use vulcan_relay::control_schema;

fn main() {
    let schema = control_schema::schema();
    println!("{}", &schema.sdl());
}
