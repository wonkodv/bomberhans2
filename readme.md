Bomberhans
==========

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/wonkodv/bomberhans2/check.yml)](https://github.com/wonkodv/bomberhans2/actions "CI Checks")
[![More Badges](https://img.shields.io/badge/Hans-on%20fire-red)](https://raw.githubusercontent.com/wonkodv/bomberhans2/main/images/hans_placing2.bmp "Hans")
[![GitHub release](https://img.shields.io/github/v/release/wonkodv/bomberhans2)](https://github.com/wonkodv/bomberhans2/releases/latest "Latest Release")
[![GitHub all releases](https://img.shields.io/github/downloads/wonkodv/bomberhans2/total)](https://github.com/wonkodv/bomberhans2/releases "releases")
![GitHub commit activity](https://img.shields.io/github/commit-activity/t/wonkodv/bomberhans2)
![GitHub repo file count](https://img.shields.io/github/directory-file-count/wonkodv/bomberhans2)
![GitHub code size in bytes](https://img.shields.io/github/languages/code-size/wonkodv/bomberhans2)
![GitHub watchers](https://img.shields.io/github/watchers/wonkodv/bomberhans2)

![screenshot](screenshot.png)



TODOS
------

*   Fit game field into Window
*   Game Modes:
    *   Destroy other's start point to win
    *   Eat schinken at center of field to win
    *   Tombstones explode with Player's schinken
    *   Tombstones give powerup's that the player lost when dying
    *   Teleports explode All other TPs
    *   Hans gets tired and needs schinken to run
    *

Client Server synchronization
-----------------------------

Client actions, tagged with a timestamp, are sent to server and local simulation.

Servers broadcast client actions which are legal in the server simulation

Clients are ahead of server by roughly 1 RTT. (~ 5 updates / frames).

Clients run 2 simulations. The assumed and the verified.
Actions are fed to the assumed, which is updated and drawn every tick.

When actions are received from the server, they are fed into the verified simulation.
The verified simulation is then cloned and fed all newer user actions and the 5 updates.
This produces the new assumed state, which will be rendered on the next frame
