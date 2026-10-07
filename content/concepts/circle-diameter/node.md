# circle-diameter

Level: isced-2
Prerequisites: circle
Rests on:
Sources: openstax-prealgebra-2e

## Way from-radius
Output: circle.diameter
Inputs: circle.radius
Relation: circle-diameter-radius
Sources: openstax-prealgebra-2e
Recognized: no
```calc
r |-> 2 * r
```
```calc
r |-> r > 0
```

## Way radius-from-diameter
Output: circle.radius
Inputs: circle.diameter
Relation: circle-diameter-radius
Sources: openstax-prealgebra-2e
Recognized: no
```calc
d |-> d / 2
```
```calc
d |-> d > 0
```
