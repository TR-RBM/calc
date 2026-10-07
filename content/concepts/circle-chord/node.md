# circle-chord

Level: isced-3
Prerequisites: circle, law-of-cosines
Rests on: M-GEO-S-030
Sources: openstax-algebra-and-trigonometry-2e

## Pattern half-angle-form
Relation: circle-chord-radius-central-angle
```calc
r |-> t |-> 2 * r * sin(t / 2)
```
```calc
r |-> t |-> r > 0 and t > 0 and t < 2 * pi
```

## Way from-radius-and-central-angle
Output: circle.chord
Inputs: circle.radius, circle.central-angle
Relation: circle-chord-radius-central-angle
Sources: openstax-algebra-and-trigonometry-2e
Recognized: yes
```calc
r |-> t |-> r * sqrt(2 - 2 * cos(t))
```
```calc
r |-> t |-> r > 0 and t > 0 and t < 2 * pi
```

## Way radius-from-chord-and-central-angle
Output: circle.radius
Inputs: circle.chord, circle.central-angle
Relation: circle-chord-radius-central-angle
Sources: openstax-algebra-and-trigonometry-2e
Recognized: yes
```calc
s |-> t |-> s / sqrt(2 - 2 * cos(t))
```
```calc
s |-> t |-> s > 0 and t > 0 and t < 2 * pi
```
