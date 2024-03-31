# User Full Name

## Sketch

```typescript
export class ExamStats {
    constructor(public students: string[], 
                public grades: number[]) {}

    public factor(f: (_: number) => number): void {
        ??? 
    }
}
```

## Input - Output

input:
[
{
`this` $\mapsto$ `{"students": ["1", "2"], "grades": [70, 90]}`
`f` $\mapsto$ `(x) => Math.min(x + 5, 100)`
},
{
`this` $\mapsto$ `{"students": ["3", "4"], "grades": [80, 100]}`
`f` $\mapsto$ `(x) => Math.ceil(Math.sqrt(x) * 10)`
}
]

output:
[
`this` $\mapsto$ `{"students": ["1", "2"], "grades": [75, 95]}`
`this` $\mapsto$ `{"students": ["3", "4"], "grades": [90, 100]}`,
]

## Solution

```typescript
export class ExamStats {
    constructor(public students: string[], 
                public grades: number[]) {}

    public factor(f: (_: number) => number): void {
        this.grades = this.grades.map(f);
    }
}
```
