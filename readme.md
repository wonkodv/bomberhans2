Bomberhans
==========

TODOS
------

*   Make hans walk in the middle of the isle, scooting around corners like BH1
*   Bomb Placing should not interrupt walking. Bombs just appear.
*   apply Bomb offset only while walking. use State::Placing only from Standing


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
