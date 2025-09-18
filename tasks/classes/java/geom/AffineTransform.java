package geom;

/**
 * Axis-aligned affine transform parameters (independent scaling).
 *
 * To be applied as output = offset + scale * input.
 */
public class AffineTransform {
    public Vec2 offset;
    public Rect matrix;

    public AffineTransform(Vec2 offset, Rect matrix) {
        this.offset = offset;
        this.matrix = matrix;
    }

    public static AffineTransform getScaleTransform(double scaleX, double scaleY) {
        return new AffineTransform(new Vec2(0, 0), new Rect(scaleX, 0, 0, scaleY));
    }

    public static AffineTransform getShearTransform(double shrX, double shrY) {
        return new AffineTransform(new Vec2(0, 0), new Rect(1, shrX, shrY, 1));
    }

    /** Apply the transform to a point. */
    public static Vec2 transformPoint(Vec2 input, AffineTransform transform) {
        return new Vec2(
            transform.offset.getX() + transform.matrix.min.getX() * input.getX() + transform.matrix.min.getY() * input.getY(),
            transform.offset.getY() + transform.matrix.max.getX() * input.getX() + transform.matrix.max.getY() * input.getY()
        );
    }

    /** Apply the transform to a vector (linear part). */
    public static Vec2 transformVec(Vec2 input, AffineTransform transform) {
        AffineTransform linear = new AffineTransform(new Vec2(0, 0), transform.matrix);
        return transformPoint(input, linear);
    }

    /** Apply the transform to a rect. */
    public static Rect transformRect(Rect input, AffineTransform transform) {
        // Some trickery needed to ensure we don't end up with an upside down rect.
        return Rect.boundingBox(
            transformPoint(input.min, transform),
            transformPoint(input.max, transform)
        );
    }

    /** Apply the transform to an area (rect without an origin, linear transformation). */
    public static Rect transformArea(Rect input, AffineTransform transform) {
        return Rect.boundingBox(
            transformVec(input.min, transform),
            transformVec(input.max, transform)
        );
    }
}