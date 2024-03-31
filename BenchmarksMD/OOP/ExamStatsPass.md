# User Full Name

## Sketch

```typescript
export class ExamStats {
    constructor(public students: string[], 
                public grades: number[]) {}

    public pass_percent(passing_grade: number): number {
        ???
    }
}
```

## Input - Output

input:
[
{
`this` $\mapsto$ `{"students": ["1", "2"], "grades": [70, 90]}`,
`passing_grade` $\mapsto$ `60`
},
{
`this` $\mapsto$ `{"students": ["1", "2"], "grades": [30, 90]}`,
`passing_grade` $\mapsto$ `60`
}
]

output:
[
`100`, `50`
]

## Solution

```typescript
export class ExamStats {
    constructor(public students: string[], 
                public grades: number[]) {}

    public pass_percent(passing_grade: number): number {
        return this.grades.filter(x => x > passing_grade).length / this.grades.length * 100;
    }
}
```
