# circle-sagitta

Level: isced-3
Prerequisites: circle-chord, pythagorean-theorem
Rests on: M-FAN-S-029
Sources: openstax-prealgebra-2e

## Way radius-from-chord-and-sagitta
Output: circle.radius
Inputs: circle.chord, circle.sagitta
Relation: circle-sagitta-radius-chord
Sources: openstax-prealgebra-2e
Recognized: yes
```calc
s |-> h |-> h / 2 + s^2 / (8 * h)
```
```calc
s |-> h |-> s > 0 and h > 0
```
