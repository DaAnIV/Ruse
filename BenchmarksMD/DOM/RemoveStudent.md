## Idea:
Input output examples are in HTML and out HTML (+ context)

### DOM

```html
<document>
  <body>
    <ul id="students">
        <li>John</li>
    </ul>
  </body>
</document>
```

```html
<document>
  <body>
    <ul id="students">
    </ul>
  </body>
</document>
```

### Sketch

```typescript
remove_student(student: Student): void {
    let students = document.getElementById("students");
    ???
}
```

### Solution

```typescript
remove_student(student: Student): void {
    let students = document.getElementById("students");
    let items = Array.from(students.querySelectorAll("li"));
    let node = items.find(el => el.textContent === student.name);
    students.removeChild(node)
}
```
