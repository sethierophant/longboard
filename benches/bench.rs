use criterion::{black_box, criterion_group, criterion_main, Criterion};

use rocket::local::{Client, LocalResponse};

use longboard::{new_instance, Config};

fn get_home<'c>(client: &'c Client) -> LocalResponse<'c> {
    client.get("/").dispatch()
}

pub fn bench_homepage(c: &mut Criterion) {
    let rocket = new_instance(Config::default()).unwrap();
    let client = Client::new(rocket).expect("valid rocket instance");

    c.bench_function("home", |b| b.iter(|| get_home(black_box(&client))));
}

criterion_group!(benches, bench_homepage);
criterion_main!(benches);
