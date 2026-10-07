# circle-circumference

Level: isced-2
Prerequisites: circle-diameter
Rests on:
Sources: openstax-prealgebra-2e

## Way from-radius
Output: circle.circumference
Inputs: circle.radius
Relation: circle-circumference-radius
Sources: openstax-prealgebra-2e
Recognized: yes
```calc
r |-> 2 * pi * r
```
```calc
r |-> r > 0
```

## Way radius-from-circumference
Output: circle.radius
Inputs: circle.circumference
Relation: circle-circumference-radius
Sources: openstax-prealgebra-2e
Recognized: no
```calc
c |-> c / (2 * pi)
```
```calc
c |-> c > 0
```

## Way from-diameter
Output: circle.circumference
Inputs: circle.diameter
Relation: circle-circumference-diameter
Sources: openstax-prealgebra-2e
Recognized: no
```calc
d |-> pi * d
```
```calc
d |-> d > 0
```

## Way diameter-from-circumference
Output: circle.diameter
Inputs: circle.circumference
Relation: circle-circumference-diameter
Sources: openstax-prealgebra-2e
Recognized: no
```calc
c |-> c / pi
```
```calc
c |-> c > 0
```
