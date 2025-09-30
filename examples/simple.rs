#![allow(missing_docs)]


use bevy_eqchanged::EqChanged;
use core::time::Duration;
use std::time::Instant;
use bevy_app::{
    App,
    Startup, Update,
    ScheduleRunnerPlugin
};
use bevy_ecs::{
    component::Component,
    system::{
        Commands,
        Query
    }
};


#[derive(Component, PartialEq, Clone, Debug)]
struct A(usize);

#[derive(Component)]
struct B(Instant);


fn main() {
    App::new()
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(250)))
        .add_systems(Startup, setup_a)
        .add_systems(Update, noedit_a)
        .add_systems(Update, edit_a)
        .add_systems(Update, after_change_a0)
        .add_systems(Update, after_change_a1)
        .run();
}


fn setup_a(mut cmds : Commands) {
    cmds.spawn(A(0));
    cmds.spawn(B(Instant::now() + Duration::from_secs(1)));
}

fn noedit_a(mut q_a : Query<&mut A>) {
    for mut a in &mut q_a {
        a.0 += 0;
    }
}

fn edit_a(
    mut q_a : Query<&mut A>,
    mut q_b : Query<&mut B>
) {
    let now = Instant::now();
    if (q_b.iter().any(|b| now >= b.0)) {
        for mut a in &mut q_a {
            a.0 += 1;
        }
        let later = now + Duration::from_secs(1);
        for mut b in &mut q_b {
            b.0 = later;
        }
    }
}

fn after_change_a0(q_a : Query<&A, EqChanged<A>>) {
    println!("a0");
    for a in &q_a {
        println!("A0 {:?}", a.0);
    }
}

fn after_change_a1(q_a : Query<&A, EqChanged<A>>) {
    println!("a1");
    for a in &q_a {
        println!("A1 {:?}", a.0);
    }
}
