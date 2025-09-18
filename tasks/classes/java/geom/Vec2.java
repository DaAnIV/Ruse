package geom;

public class Vec2 {
    private double[] values = new double[2];

    public Vec2(double x, double y) {
        this.values[0] = x;
        this.values[1] = y;
    }

    public Vec2(double[] values) {
        this.values[0] = values[0];
        this.values[1] = values[1];
    }

    public Vec2(double k) {
        this.values[0] = k;
        this.values[1] = k;
    }

    public double getX() { 
        return this.values[0]; 
    }
    
    public double getY() { 
        return this.values[1]; 
    }

    public void setX(double val) { 
        this.values[0] = val; 
    }
    
    public void setY(double val) { 
        this.values[1] = val; 
    }

    public static Vec2 getLeft() { return new Vec2(-1, 0); }
    public static Vec2 getRight() { return new Vec2(+1, 0); }
    public static Vec2 getUp() { return new Vec2(0, +1); }
    public static Vec2 getDown() { return new Vec2(0, -1); }

    public static Vec2 getZero() { return new Vec2(0, 0); }
    public static Vec2 getOne() { return new Vec2(1, 1); }

    public static Vec2 getX() { return new Vec2(+1, 0); }
    public static Vec2 getY() { return new Vec2(0, +1); }

    /** Adds the scalar value to each component. */
    public Vec2 add(double scalar) {
        return new Vec2(this.values[0] + scalar, this.values[1] + scalar);
    }

    /** Returns the vector addition (this + vector). */
    public Vec2 add(Vec2 vector) {
        return new Vec2(this.values[0] + vector.values[0], this.values[1] + vector.values[1]);
    }

    /** Subtracts the scalar value from each component. */
    public Vec2 sub(double scalar) {
        return new Vec2(this.values[0] - scalar, this.values[1] - scalar);
    }

    /** Returns the vector subtraction (this - that). */
    public Vec2 sub(Vec2 that) {
        return new Vec2(this.values[0] - that.values[0], this.values[1] - that.values[1]);
    }

    /** Returns the dot product (this * that). */
    public double dot(Vec2 that) {
        return this.values[0] * that.values[0] + this.values[1] * that.values[1];
    }

    /**
     * Returns 2D cross product (this x that).
     *
     * Equivalent to embedding this and that in the XY plane and returning the Z value of the product vector
     * (such a vector would be of the form (0, 0, z)).
     */
    public double cross(Vec2 that) {
        return this.values[0] * that.values[1] - this.values[1] * that.values[0];
    }

    /** Returns the scalar product (scalar * this). */
    public Vec2 times(double scalar) {
        return new Vec2(this.values[0] * scalar, this.values[1] * scalar);
    }

    /** Returns the component-wise product (this * vector). */
    public Vec2 times(Vec2 vector) {
        return new Vec2(this.values[0] * vector.values[0], this.values[1] * vector.values[1]);
    }

    /** Returns the scalar division (this / scalar). */
    public Vec2 div(double scalar) {
        return new Vec2(this.values[0] / scalar, this.values[1] / scalar);
    }

    /** Returns the component-wise division (this / vector). */
    public Vec2 div(Vec2 vector) {
        return new Vec2(this.values[0] / vector.values[0], this.values[1] / vector.values[1]);
    }

    /** Returns -this. */
    public Vec2 negate() {
        return this.times(-1);
    }

    /** Returns the squared magnitude of this vector. */
    public double magSqr() {
        return this.values[0] * this.values[0] + this.values[1] * this.values[1];
    }

    /** Returns the magnitude of this vector. */
    public double mag() {
        return Math.sqrt(this.magSqr());
    }

    /** Returns a normalized copy of this vector. */
    public Vec2 normalized() {
        return this.div(this.mag());
    }

    /**
     * Returns the angle between this vector and the x-axis.
     *
     * Returns the angle between this vector and (1, 0), in radians, in the range (-Pi, +Pi].
     */
    public double argument() {
        return Math.atan2(this.values[1], this.values[0]);
    }

    /** Returns a copy of this vector. */
    public Vec2 clone() {
        return new Vec2(this.values[0], this.values[1]);
    }

    /** Returns a copy of this vector, scaled if needed so its magnitude is at most 'length'. */
    public Vec2 cap(double length) {
        if (length <= Double.MIN_VALUE) {
            return new Vec2(0, 0);
        }
        double mag = this.mag();
        if (length < mag) {
            return this.times(length / mag);
        }
        return this.clone();
    }

    /** Returns a copy of this vector, swapping x and y. */
    public Vec2 transpose() {
        return new Vec2(this.values[1], this.values[0]);
    }

    /** Returns the orthogonal vector v such that (this, v) is a right-handed basis, and |v| = |this|. */
    public Vec2 orthogonal() {
        return new Vec2(-this.values[1], this.values[0]);
    }

    /** Returns a copy of this vector, applying floor() to all components. */
    public Vec2 floor() {
        return new Vec2(Math.floor(this.values[0]), Math.floor(this.values[1]));
    }

    /** Returns a copy of this vector, applying ceil() to all components. */
    public Vec2 ceil() {
        return new Vec2(Math.ceil(this.values[0]), Math.ceil(this.values[1]));
    }

    /** Returns a copy of this vector, applying abs() to all components. */
    public Vec2 abs() {
        return new Vec2(Math.abs(this.values[0]), Math.abs(this.values[1]));
    }

    /** Returns the maximum component in this vector. */
    public double max() {
        return Math.max(this.values[0], this.values[1]);
    }

    /** Returns the component-wise maximum of this and that. */
    public Vec2 max(Vec2 that) {
        return new Vec2(
            Math.max(this.values[0], that.values[0]),
            Math.max(this.values[1], that.values[1])
        );
    }

    /** Returns the minimum component in this vector. */
    public double min() {
        return Math.min(this.values[0], this.values[1]);
    }

    /** Returns the component-wise minimum of this and that. */
    public Vec2 min(Vec2 that) {
        return new Vec2(
            Math.min(this.values[0], that.values[0]),
            Math.min(this.values[1], that.values[1])
        );
    }

    /** Returns the Euclidean distance between u and v. */
    public static double dist(Vec2 u, Vec2 v) {
        return u.sub(v).mag();
    }

    /** Returns a Vec2 (Cartesian coordinates) corresponding to the polar coordinates (radius, angle). */
    public static Vec2 fromPolar(double radius, double angle) {
        return new Vec2(radius * Math.cos(angle), radius * Math.sin(angle));
    }

    /** Linearly interpolate between a at t=0 and b at t=1 (t is NOT clamped). */
    public static Vec2 interpolate(Vec2 a, Vec2 b, double t) {
        return a.add(b.sub(a).times(t));
    }

    /** Calculate the average vector. */
    public static Vec2 average(Vec2... vecs) {
        Vec2 accumulator = new Vec2(0, 0);
        if (vecs.length == 0) {
            return accumulator;
        }

        for (Vec2 vec : vecs) {
            accumulator = accumulator.add(vec);
        }

        return accumulator.div(vecs.length);
    }

    /**
     * Calculate the weighted average vector.
     *
     * * Iterates up to shortest length.
     * * Ignores negative or approximately zero weights and their associated vectors.
     */
    public static Vec2 weightedAverage(Vec2[] vecs, double[] weights) {
        Vec2 accumulator = new Vec2(0, 0);
        double totalWeight = 0;

        int N = Math.min(vecs.length, weights.length);
        if (N == 0) {
            return accumulator;
        }

        for (int i = 0; i < N; i++) {
            Vec2 vec = vecs[i];
            double weight = weights[i];
            if (weight > Double.MIN_VALUE) {
                totalWeight += weight;
                accumulator = accumulator.add(vec.times(weight));
            }
        }

        if (totalWeight > Double.MIN_VALUE) {
            return accumulator.div(totalWeight);
        } else {
            return accumulator;
        }
    }

    /** Returns the projection of arbitrary vector 'v' into *unit* vector 'n', as a Vec2. */
    public static Vec2 project(Vec2 v, Vec2 n) {
        return n.times(v.dot(n));
    }
}