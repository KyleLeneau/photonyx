# DATA Corpus for testing

Uses real data captured from a telescope to have real results in benchmarks.

### Processing Steps for Both Sets

* master bias - stack the raw images
* master dark - stack of raw images
* master flat - calibrate frames by removing master bias first
* calibrated light - using master dark and master flat (with bias already removed)
* master light - stack of calibrated & aligned (registered) lights

## FRA 300 Mono (6200mm pro) - 300s - H filter - Veil Nebula

src: `'/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/Veil Nebula'`

All Raw frames
* corpus-mono/bias/   ~20-50 frames
* corpus-mono/dark/   ~20-50, matching exposure + temperature
* corpus-mono/flat/   ~20-30, single filter
* corpus-mono/light/  ~50-100, one target/filter/session


## WO 91 OSC (6200mc pro) - 300s - L-Ultimate filter - NGC 7000

src: `/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/NGC_7000`

All Raw frames
* corpus-mono/bias/   ~20-50 frames
* corpus-mono/dark/   ~20-50, matching exposure + temperature
* corpus-mono/flat/   ~20-30, single filter
* corpus-mono/light/  ~50-100, one target/filter/session
