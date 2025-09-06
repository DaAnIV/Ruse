export class BinarySearchTreeNode {
    private parent?: BinarySearchTreeNode;
    private is_left: boolean;
    private _height: number
    private _size: number

    constructor(value: number);
    constructor(value: number, left?: BinarySearchTreeNode, right?: BinarySearchTreeNode);
    constructor(private _value: number, private _left?: BinarySearchTreeNode | undefined, private _right?: BinarySearchTreeNode | undefined) {
        this._size = 1;
        this._height = 1;
        if (this._right != null) {
            this._size += this._right.size;
            this._height = this._right.height + 1;
            this._right.parent = this;
            this._right.is_left = false
        }
        if (this._left != null) {
            this._size += this._left.size;
            if (this._left.height + 1 > this._height) {
                this._height = this._left.height + 1;
            }
            this._left.parent = this;
            this._left.is_left = true;
        }
    }

    public get right(): BinarySearchTreeNode | undefined {
        return this._right;
    }

    public get left(): BinarySearchTreeNode | undefined {
        return this._left;
    }

    public get size(): number {
        return this._size;
    }

    public get height(): number {
        return this._height;
    }

    public get value(): number {
        return this._value;
    }

    public min_node(): BinarySearchTreeNode {
        let node: BinarySearchTreeNode | undefined = this;
        while (node.left != null) {
            node = node.left;
        }

        return node;
    }

    private max_node(): BinarySearchTreeNode {
        let node: BinarySearchTreeNode | undefined = this;
        while (node.right != null) {
            node = node.right;
        }

        return node;
    }

    public swap(other: BinarySearchTreeNode): BinarySearchTreeNode {
        let val = this.value;
        this._value = other.value;
        other._value = val;
        return this
    }

    // Function to check if the tree is a valid BST
    public valid(): boolean {
        function inorder(root, prev) {
            if (root === null)
                return true;

            // Recursively check the left subtree
            if (!inorder(root.left, prev))
                return false;

            // Check the current node value against the previous value
            if (prev[0] >= root.value)
                return false;

            // Update the previous value to the current node's value
            prev[0] = root.value;

            // Recursively check the right subtree
            return inorder(root.right, prev);
        }

        let prev = [-Infinity];
        return inorder(this, prev);
    }

    public unlink_leaf(): void {
        if (this.left != null || this.right != null) {
            throw new Error("Not a leaf");
        }
        if (this.parent == null) {
            return
        }

        if (this.is_left) {
            this.parent._left = undefined;
        } else {
            this.parent._right = undefined;
        }

        let cur_parent: BinarySearchTreeNode | undefined = this.parent;
        while (cur_parent != null) {
            cur_parent._size -= 1;
            if (cur_parent._right != null) {
                cur_parent._height = cur_parent._right.height + 1;
            }
            if (cur_parent._left != null) {
                if (cur_parent._left.height + 1 > cur_parent._height) {
                    cur_parent._height = cur_parent._left.height + 1;
                }
            }
            cur_parent = cur_parent.parent;
        }
    }

    public contains(value: number): boolean {
        let node: BinarySearchTreeNode | undefined = this;
        while (node != null) {
            if (node.value == value) {
                return true;
            } else if (value < node.value) {
                node = node.left;
            } else {
                node = node.right;
            }
        }
        return false;
    }
}
