package ruse;

public class BinarySearchTreeNode {
    private BinarySearchTreeNode parent;
    private boolean isLeft;
    private int height;
    private int size;
    private double value;
    private BinarySearchTreeNode left;
    private BinarySearchTreeNode right;

    public BinarySearchTreeNode(double value) {
        this(value, null, null);
    }

    public BinarySearchTreeNode(double value, BinarySearchTreeNode left, BinarySearchTreeNode right) {
        this.value = value;
        this.left = left;
        this.right = right;
        this.size = 1;
        this.height = 1;
        
        if (this.right != null) {
            this.size += this.right.size;
            this.height = this.right.height + 1;
            this.right.parent = this;
            this.right.isLeft = false;
        }
        if (this.left != null) {
            this.size += this.left.size;
            if (this.left.height + 1 > this.height) {
                this.height = this.left.height + 1;
            }
            this.left.parent = this;
            this.left.isLeft = true;
        }
    }

    public BinarySearchTreeNode getRight() {
        return this.right;
    }

    public BinarySearchTreeNode getLeft() {
        return this.left;
    }

    public int getSize() {
        return this.size;
    }

    public int getHeight() {
        return this.height;
    }

    public double getValue() {
        return this.value;
    }

    public BinarySearchTreeNode minNode() {
        BinarySearchTreeNode node = this;
        while (node.left != null) {
            node = node.left;
        }
        return node;
    }

    private BinarySearchTreeNode maxNode() {
        BinarySearchTreeNode node = this;
        while (node.right != null) {
            node = node.right;
        }
        return node;
    }

    public BinarySearchTreeNode swap(BinarySearchTreeNode other) {
        double val = this.value;
        this.value = other.value;
        other.value = val;
        return this;
    }

    // Function to check if the tree is a valid BST
    public boolean valid() {
        double[] prev = {Double.NEGATIVE_INFINITY};
        return inorder(this, prev);
    }

    private boolean inorder(BinarySearchTreeNode root, double[] prev) {
        if (root == null)
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

    public void unlinkLeaf() {
        if (this.left != null || this.right != null) {
            throw new RuntimeException("Not a leaf");
        }
        if (this.parent == null) {
            return;
        }

        if (this.isLeft) {
            this.parent.left = null;
        } else {
            this.parent.right = null;
        }

        BinarySearchTreeNode curParent = this.parent;
        while (curParent != null) {
            curParent.size -= 1;
            if (curParent.right != null) {
                curParent.height = curParent.right.height + 1;
            }
            if (curParent.left != null) {
                if (curParent.left.height + 1 > curParent.height) {
                    curParent.height = curParent.left.height + 1;
                }
            }
            curParent = curParent.parent;
        }
    }

    public boolean contains(double value) {
        BinarySearchTreeNode node = this;
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