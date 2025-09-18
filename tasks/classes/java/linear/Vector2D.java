package linear;

public class Vector2D {
    public final double x;
    public final double y;

    public Vector2D(double x, double y) {
        this.x = x;
        this.y = y;
    }

    /**
     * Create a vector from polar coordinates
     */
    public static Vector2D fromPolar(double magnitude, double angle) {
        return new Vector2D(
            magnitude * Math.cos(angle),
            magnitude * Math.sin(angle)
        );
    }

    /**
     * Create a zero vector
     */
    public static Vector2D zero() {
        return new Vector2D(0, 0);
    }

    /**
     * Create a unit vector in the x direction
     */
    public static Vector2D unitX() {
        return new Vector2D(1, 0);
    }

    /**
     * Create a unit vector in the y direction
     */
    public static Vector2D unitY() {
        return new Vector2D(0, 1);
    }

    /**
     * Get the magnitude (length) of the vector
     */
    public double magnitude() {
        return Math.sqrt(this.x * this.x + this.y * this.y);
    }

    /**
     * Get the squared magnitude (faster than magnitude for comparisons)
     */
    public double magnitudeSquared() {
        return this.x * this.x + this.y * this.y;
    }

    /**
     * Normalize the vector (make it unit length)
     */
    public Vector2D normalize() {
        double mag = this.magnitude();
        if (mag == 0) return new Vector2D(0, 0);
        return new Vector2D(this.x / mag, this.y / mag);
    }

    /**
     * Add another vector to this one
     */
    public Vector2D add(Vector2D other) {
        return new Vector2D(this.x + other.x, this.y + other.y);
    }

    /**
     * Subtract another vector from this one
     */
    public Vector2D subtract(Vector2D other) {
        return new Vector2D(this.x - other.x, this.y - other.y);
    }

    /**
     * Multiply this vector by a scalar
     */
    public Vector2D multiply(double scalar) {
        return new Vector2D(this.x * scalar, this.y * scalar);
    }

    /**
     * Divide this vector by a scalar
     */
    public Vector2D divide(double scalar) {
        if (scalar == 0) throw new IllegalArgumentException("Cannot divide by zero");
        return new Vector2D(this.x / scalar, this.y / scalar);
    }

    /**
     * Calculate the dot product with another vector
     */
    public double dot(Vector2D other) {
        return this.x * other.x + this.y * other.y;
    }

    /**
     * Calculate the cross product with another vector (returns scalar in 2D)
     */
    public double cross(Vector2D other) {
        return this.x * other.y - this.y * other.x;
    }

    /**
     * Calculate the angle between this vector and another
     */
    public double angleTo(Vector2D other) {
        double dot = this.dot(other);
        double mag1 = this.magnitude();
        double mag2 = other.magnitude();
        if (mag1 == 0 || mag2 == 0) return 0;
        return Math.acos(dot / (mag1 * mag2));
    }

    /**
     * Calculate the distance between this vector and another
     */
    public double distanceTo(Vector2D other) {
        return this.subtract(other).magnitude();
    }

    /**
     * Calculate the squared distance between this vector and another
     */
    public double distanceSquaredTo(Vector2D other) {
        return this.subtract(other).magnitudeSquared();
    }

    /**
     * Rotate the vector by an angle (in radians)
     */
    public Vector2D rotate(double angle) {
        double cos = Math.cos(angle);
        double sin = Math.sin(angle);
        return new Vector2D(
            this.x * cos - this.y * sin,
            this.x * sin + this.y * cos
        );
    }

    /**
     * Get the angle of the vector (in radians)
     */
    public double angle() {
        return Math.atan2(this.y, this.x);
    }

    /**
     * Check if this vector equals another vector
     */
    public boolean equals(Vector2D other) {
        return equals(other, 1e-10);
    }

    public boolean equals(Vector2D other, double epsilon) {
        return Math.abs(this.x - other.x) < epsilon && 
               Math.abs(this.y - other.y) < epsilon;
    }

    /**
     * Check if this vector is approximately zero
     */
    public boolean isZero() {
        return isZero(1e-10);
    }

    public boolean isZero(double epsilon) {
        return this.magnitudeSquared() < epsilon * epsilon;
    }

    /**
     * Create a copy of this vector
     */
    public Vector2D clone() {
        return new Vector2D(this.x, this.y);
    }

    /**
     * Convert to string representation
     */
    @Override
    public String toString() {
        return "Vector2D(" + this.x + ", " + this.y + ")";
    }

    /**
     * Convert to array
     */
    public double[] toArray() {
        return new double[]{this.x, this.y};
    }

    /**
     * Create from array
     */
    public static Vector2D fromArray(double[] arr) {
        return new Vector2D(arr[0], arr[1]);
    }
}