# triangle-area

Level: isced-2
Prerequisites: triangle
Rests on:
Sources: openstax-prealgebra-2e, openstax-algebra-and-trigonometry-2e

## Way base-height
Output: triangle.area
Inputs: triangle.side-b, triangle.height-b
Relation: triangle-area-base-height
Sources: openstax-prealgebra-2e
Recognized: no
```calc
b |-> h |-> b * h / 2
```
```calc
b |-> h |-> b > 0 and h > 0
```

## Way two-sides-angle
Output: triangle.area
Inputs: triangle.side-a, triangle.side-b, triangle.angle-gamma
Relation: triangle-area-two-sides-angle
Sources: openstax-algebra-and-trigonometry-2e
Recognized: yes
```calc
a |-> b |-> g |-> a * b * sin(g) / 2
```
```calc
a |-> b |-> g |-> a > 0 and b > 0 and g > 0 and g < pi
```

## Bound two-sides
Role: triangle.area
Inputs: triangle.side-a, triangle.side-b
Relation kind: at-most
Sources: openstax-algebra-and-trigonometry-2e
```calc
a |-> b |-> a * b / 2
```
```calc
a |-> b |-> a > 0 and b > 0
```
