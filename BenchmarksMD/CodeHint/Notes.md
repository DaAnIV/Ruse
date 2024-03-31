# Notes

## Task 1

Base64 encoding/decoding.

Not really interesting in javascript.
It is just a function call on a string.

## Task 2

Get element index in list.

Can perform on DOM list (Without jquery which no one does but whatever)

<https://stackoverflow.com/questions/18295673/javascript-find-li-index-within-ul>

### getClickedIndex

```javascript
let ul = document.getElementById('notesList');
ul.onclick = function(event) {
    ???
};
```

```javascript
let ul = document.getElementById('notesList');
ul.onclick = function(event) {
    let target = e.target;
    let li = target.closest('li'); // get reference by using closest
    let nodes = Array.from( ul.children ); // get array
    // let nodes = [...ul.childNodes]; // Or using spread operator
    let index = nodes.indexOf( li ); 
};    
```

### getClickedNoteTitle

This is quite different but we can emulate something like the model
And then it is just getting the correct value

## Task 3

We can make something similiar but I think it is quite difficult.

We can do get selection and modify text, For example:

```javascript
let s = window.getSelection();
let node = s.focusNode;
let before = node.data.substr(0, s.baseOffset);
let after = node.data.substr(s.extentOffset)
node.data = before + s.toString().toUpperCase() + after;
```

It does seem quite hard

Regarding menu items and commands
We can do that with `<ul>` and `<li>.onclick`
