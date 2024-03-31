## Idea:
Input output examples are in HTML and out HTML (+ context)

### DOM

```html
<document>
  <body>
    <form id="new_student">
      <label for="fname">First name:</label><br>
      <input type="text" id="fname" name="fname" value="John"><br>
      <label for="lname">Last name:</label><br>
      <input type="text" id="lname" name="lname" value="Doe"><br><br>
      <input type="submit" value="Submit">
    </form> 
  </body>
</document>
```

```html
<document>
  <body>
    <form id="new_student">
      <label for="fname">First name:</label><br>
      <input type="text" id="fname" name="fname" value="John"><br>
      <label for="lname">Last name:</label><br>
      <input type="text" id="lname" name="lname" value="Doe"><br><br>
      <input type="submit" value="Submit">
    </form> 
  </body>
</document>
```

### Sketch

```typescript
class Student {
  constructor(public fname: string, public lname: string) {}
}

const form = document.getElementById("new_student");
form.addEventListener('submit', (event) => {
  // stop form submission
  event.preventDefault();
    
  let s = ???
  console.log(s);
});
```

### Solution

```typescript
class Student {
  constructor(public fname: string, public lname: string) {}
}

const form = document.getElementById("new_student");
form.addEventListener('submit', (event) => {
  // stop form submission
  event.preventDefault();
      
  let fname = form.elements["fname"];
  let lname = form.elements["lname"];
  let s = new Student(fname.value, lname.value); 
  console.log(s);
});
```
