### DOM

```html
<document>
  <body>
  </body>
</document>
```

```html
<document>
  <body>
    <h1>Big Head!</h1>
  </body>
</document>
```

### Sketch

```typescript
// run this function when the document is loaded
window.onload = () => {
    ???
};
```

### Solution

```typescript
// run this function when the document is loaded
window.onload = () => {
    // create a couple of elements in an otherwise empty HTML page
    const heading = document.createElement("h1");
    const headingText = document.createTextNode("Big Head!");
    heading.appendChild(headingText);
    document.body.appendChild(heading);
};
```
