export class Vector2D {
  constructor(public x: number, public y: number) {}

  /**
   * Create a vector from polar coordinates
   */
  static fromPolar(magnitude: number, angle: number): Vector2D {
    return new Vector2D(
      magnitude * Math.cos(angle),
      magnitude * Math.sin(angle)
    );
  }

  /**
   * Create a zero vector
   */
  static zero(): Vector2D {
    return new Vector2D(0, 0);
  }

  /**
   * Create a unit vector in the x direction
   */
  static unitX(): Vector2D {
    return new Vector2D(1, 0);
  }

  /**
   * Create a unit vector in the y direction
   */
  static unitY(): Vector2D {
    return new Vector2D(0, 1);
  }

  /**
   * Get the magnitude (length) of the vector
   */
  magnitude(): number {
    return Math.sqrt(this.x * this.x + this.y * this.y);
  }

  /**
   * Get the squared magnitude (faster than magnitude for comparisons)
   */
  magnitudeSquared(): number {
    return this.x * this.x + this.y * this.y;
  }

  /**
   * Normalize the vector (make it unit length)
   */
  normalize(): Vector2D {
    const mag = this.magnitude();
    if (mag === 0) return new Vector2D(0, 0);
    return new Vector2D(this.x / mag, this.y / mag);
  }

  /**
   * Add another vector to this one
   */
  add(other: Vector2D): Vector2D {
    return new Vector2D(this.x + other.x, this.y + other.y);
  }

  /**
   * Subtract another vector from this one
   */
  subtract(other: Vector2D): Vector2D {
    return new Vector2D(this.x - other.x, this.y - other.y);
  }

  /**
   * Multiply this vector by a scalar
   */
  multiply(scalar: number): Vector2D {
    return new Vector2D(this.x * scalar, this.y * scalar);
  }

  /**
   * Divide this vector by a scalar
   */
  divide(scalar: number): Vector2D {
    if (scalar === 0) throw new Error("Cannot divide by zero");
    return new Vector2D(this.x / scalar, this.y / scalar);
  }

  /**
   * Calculate the dot product with another vector
   */
  dot(other: Vector2D): number {
    return this.x * other.x + this.y * other.y;
  }

  /**
   * Calculate the cross product with another vector (returns scalar in 2D)
   */
  cross(other: Vector2D): number {
    return this.x * other.y - this.y * other.x;
  }

  /**
   * Calculate the angle between this vector and another
   */
  angleTo(other: Vector2D): number {
    const dot = this.dot(other);
    const mag1 = this.magnitude();
    const mag2 = other.magnitude();
    if (mag1 === 0 || mag2 === 0) return 0;
    return Math.acos(dot / (mag1 * mag2));
  }

  /**
   * Calculate the distance between this vector and another
   */
  distanceTo(other: Vector2D): number {
    return this.subtract(other).magnitude();
  }

  /**
   * Calculate the squared distance between this vector and another
   */
  distanceSquaredTo(other: Vector2D): number {
    return this.subtract(other).magnitudeSquared();
  }

  /**
   * Rotate the vector by an angle (in radians)
   */
  rotate(angle: number): Vector2D {
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    return new Vector2D(
      this.x * cos - this.y * sin,
      this.x * sin + this.y * cos
    );
  }

  /**
   * Get the angle of the vector (in radians)
   */
  angle(): number {
    return Math.atan2(this.y, this.x);
  }

  /**
   * Check if this vector equals another vector
   */
  equals(other: Vector2D, epsilon: number = 1e-10): boolean {
    return Math.abs(this.x - other.x) < epsilon && 
           Math.abs(this.y - other.y) < epsilon;
  }

  /**
   * Check if this vector is approximately zero
   */
  isZero(epsilon: number = 1e-10): boolean {
    return this.magnitudeSquared() < epsilon * epsilon;
  }

  /**
   * Create a copy of this vector
   */
  clone(): Vector2D {
    return new Vector2D(this.x, this.y);
  }

  /**
   * Convert to string representation
   */
  toString(): string {
    return `Vector2D(${this.x}, ${this.y})`;
  }

  /**
   * Convert to array
   */
  toArray(): [number, number] {
    return [this.x, this.y];
  }

  /**
   * Create from array
   */
  static fromArray(arr: [number, number]): Vector2D {
    return new Vector2D(arr[0], arr[1]);
  }
}
