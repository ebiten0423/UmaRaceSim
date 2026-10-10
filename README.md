This is a 1:1 Umamusume: Pretty Derby Global race simulator.

Given the correct configuration and seed, it should simulate the exact same race as the real server does.

While it is perfect on the 2379/2379 races I simulated it against, there may be extremely rare weird one offs where it does diverge, feel free to make a issue to report those.

## Build

```sh
cargo build --release
```

## Run

```sh
target/release/global_race_sim simulate \
  --input examples/race.json \
  --master data/master.json \
  --assets data/assets.json \
  --output result.json
```

- `--input-dir DIR --output DIR` simulates every `.json` race in a directory (multi-threaded).
- `serve --master FILE --assets FILE` reads one race JSON per stdin line and writes one result per stdout line.
- `--max-ticks N` bounds the simulation (default 20000).

`data/master.json` holds the master.mdb tables; `data/assets.json` holds the Global RaceParamDefine, course and lane assets.
