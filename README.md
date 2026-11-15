This is a 1:1 Umamusume: Pretty Derby race simulator.

Given the correct configuration and seed, it should simulate the exact same race as the real server does.
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
