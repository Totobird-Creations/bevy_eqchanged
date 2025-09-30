# `bevy_eqchanged`
A simple library for [Bevy](https://github.com/bevyengine/bevy) to detect true changes to components.


### Why?
Bevy has the [`Changed<T>`](https://docs.rs/bevy_ecs/latest/bevy_ecs/query/struct.Changed.html) query filter.
 However, it doesn't detect *true* changes to components. Simply *mutably dereferencing*
 ([`DerefMut`](https://doc.rust-lang.org/nightly/core/ops/deref/trait.DerefMut.html)) a component is
 considered a 'change'.

[`EqChanged<T>`](https://docs.rs/bevy_eqchanged/latest/bevy_eqchanged/struct.EqChanged.html) will instead keep
 track of the 'previous value' of a component, and only pass the filter if the previous and new values are
 [`!=`](https://doc.rust-lang.org/stable/std/cmp/trait.PartialEq.html#method.ne).

<div class="warning">

Please note that `EqChanged` uses `Clone` to keep track of the previous value.
`Changed` should be preferred over `EqChanged` when possible.

</div>


### Example
Here is a crude example of where this might be useful.
The game would have a debug console where cheat commands could be entered.
```rust
#[derive(Component)]
enum PlayerMode {
    Spectator,
    Builder
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Update, run_change_mode_cheats)
        .add_systems(Update, on_mode_changed)
        .add_systems(Update, on_mode_eqchanged)
        .run();
}

fn run_change_mode_cheats(
    mut players      : Query<&mut PlayerMode>,
    mut cheat_events : EventReader<CheatCommand>
) {
    for cheat in cheat_events.read() {
        if let CheatCommand::ChangePlayerMode(player_entity, new_player_mode) = command {
            let mut player_mode = player.get_mut(player_entity).unwrap();
            *player_mode = new_player_mode; // 'Changed' here.
        }
    }
}

fn on_mode_changed(
    players : Query<(Entity, &PlayerMode,), Changed<PlayerMode>>
) {
    for (player_entity, player_mode,) in &players {
        println!("Player {} changed mode to {}.", player_entity, player_mode);
    }
}

fn on_mode_eqchanged(
    players : Query<(Entity, &PlayerMode,), EqChanged<PlayerMode>>
) {
    for (player_entity, player_mode,) in &players {
        println!("Player {} EQchanged mode to {}.", player_entity, player_mode);
    }
}
```
`on_mode_changed`'s query will hit the player *every* time the change player mode command is run.
However, `on_mode_eqchanged`'s query will only hit the player if the old player mode and new player mode were not equal.
