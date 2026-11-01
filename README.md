# Horizon-Lite

A minimal **Minecraft 1.0 client in Rust** using **glow + glutin** (OpenGL 3.3).  
Built for low-end GPUs (target: NVIDIA GTX 550 Ti).  
Forked from **Horizon 26.3** with the wgpu renderer removed and replaced by a custom OpenGL one.

## Status

| Feature | State |
|---|---|
| Window + OpenGL 3.3 context | working |
| Menu with buttons (Singleplayer, Options, Quit) | working |
| Select World screen | working |
| Options (fullscreen, render distance) | working |
| 10 blocks with correct per-face textures | working |
| Hotbar | working |
| Real chunks (16x16x256) | not yet |
| World generation | not yet |
| Player physics / collision | not yet |
| Mobs, entities | not yet |
| HUD (health, hunger) | not yet |

## Running

Requires in the working directory: acts.json, 	extures/, gui/, ont/default.png.
python gen_world_data.py
cargo run -p horizon-client
## Help wanted

Stuck on the exact acts.json format expected by
horizon_world::registry::BlockRegistry::from_facts_json
(states / defaults / lags and the Shape enum).

## License

MIT - see LICENSE. Textures and game assets are property of Mojang. This project is not affiliated with Mojang.
