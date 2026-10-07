# circle-area

Level: isced-2
Prerequisites: circle
Rests on: M-VEK-S-065
Sources: openstax-prealgebra-2e

## Pattern from-diameter
Relation: circle-area-radius
```calc
d |-> pi * d^2 / 4
```
```calc
d |-> d > 0
```

## Way from-radius
Output: circle.area
Inputs: circle.radius
Relation: circle-area-radius
Sources: openstax-prealgebra-2e
Recognized: yes
```calc
r |-> pi * r^2
```
```calc
r |-> r > 0
```

## Way radius-from-area
Output: circle.radius
Inputs: circle.area
Relation: circle-area-radius
Sources: openstax-prealgebra-2e
Recognized: yes
```calc
a |-> sqrt(a / pi)
```
```calc
a |-> a > 0
```
