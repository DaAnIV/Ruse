import { Vec2 } from './vec2';
import { Rect, BoundingBox } from './rect';

/**
 * Axis-aligned affine transform parameters (independent scaling).
 *
 * To be applied as output = offset + scale * input.
 */
// export type AffineTransform = { offset: Vec2, scale: Vec2 };
export class AffineTransform {
    constructor(public offset: Vec2, public matrix: Rect) {}

    public static getScaleTransform(scaleX: number, scaleY: number): AffineTransform {
        return new AffineTransform(new Vec2(0), new Rect(scaleX, 0, 0, scaleY));
    }

    public static getShearTransform(shrX: number, shrY: number): AffineTransform {
        return new AffineTransform(new Vec2(0), new Rect(1, shrX, shrY, 1));
    }
}

/** Apply the transform to a point. */
export const TransformPoint = (input: Vec2, transform: AffineTransform): Vec2 => {
    return new Vec2(
        transform.offset.x + transform.matrix.min.x * input.x + transform.matrix.min.y * input.y,
        transform.offset.y + transform.matrix.max.x * input.x + transform.matrix.max.y * input.y
    );
};

/** Apply the transform to a vector (linear part). */
export const TransformVec = (input: Vec2, transform: AffineTransform): Vec2 => {
    let linear = new AffineTransform(new Vec2(0), transform.matrix);
    return TransformPoint(input, linear);
};

/** Apply the transform to a rect. */
export const TransformRect = (input: Rect, transform: AffineTransform): Rect => {
    // Some trickery needed to ensure we don't end up with an upside down rect.
    return BoundingBox(
        TransformPoint(input.min, transform),
        TransformPoint(input.max, transform)
    );
};

/** Apply the transform to an area (rect without an origin, linear transformation). */
export const TransformArea = (input: Rect, transform: AffineTransform): Rect => {
    return BoundingBox(
        TransformVec(input.min, transform),
        TransformVec(input.max, transform)
    );
};

// /** Apply the inverse transform to a point. */
// export const InverseTransformPoint = (output: Vec2, transform: AffineTransform): Vec2 => {
//     return new Vec2(
//         (output.x - transform.offset.x) / transform.scale.x,
//         (output.y - transform.offset.y) / transform.scale.y
//     );
// };

// /** Apply the inverse transform to a vector (linear part). */
// export const InverseTransformVec = (output: Vec2, transform: AffineTransform): Vec2 => {
//     return new Vec2(output.x / transform.scale.x, output.y / transform.scale.y);
// };

// /** Apply the inverse transform to a rect. */
// export const InverseTransformRect = (output: Rect, transform: AffineTransform): Rect => {
//     // Some trickery needed to ensure we don't end up with an upside down rect.
//     return BoundingBox(
//         InverseTransformPoint(output.min, transform),
//         InverseTransformPoint(output.max, transform)
//     );
// };

// /** Apply the inverse transform to an area (rect without an origin, linear transformation). */
// export const InverseTransformArea = (input: Rect, transform: AffineTransform): Rect => {
//     return BoundingBox(
//         InverseTransformVec(input.min, transform),
//         InverseTransformVec(input.max, transform)
//     );
// };

// /**
//  * Additional options for ScaleStretch().
//  *
//  * zoom: modifier applied to scale after exact fit calculation (2.0 twice as big, 0.5 twice as small).
//  */
// // export type StretchOptions = {
// //     zoom?: number,
// // };
// export class StretchOptions {
//     constructor(public zoom?: number) {}
// }

// /** Calculates AffineTransform parameters (offset, scale) used to map points in inner to points in outer. */
// export const ScaleStretch = (inner: Rect, outer: Rect, options: StretchOptions = {}): AffineTransform => {
//     const zoom = (options.zoom === undefined) ? 1.0 : options.zoom;

//     const diagIn  = inner.Diagonal();
//     const diagOut = outer.Diagonal();

//     const ci = inner.Center();
//     const co = outer.Center();

//     const sx = zoom * diagOut.x / diagIn.x;
//     const sy = zoom * diagOut.y / diagIn.y;

//     const ox = co.x - sx * ci.x;
//     const oy = co.y - sy * ci.y;

//     return { offset: new Vec2(ox, oy), scale: new Vec2(sx, sy) };
// }

// /** Uniform scaling transform parameters. */
// // export type UniformTransform = { offset: Vec2, scale: number, invertY: boolean };
// export class UniformTransform {
//     constructor(public offset: Vec2, public scale: number, public invertY: boolean) {}
// }

// /**
//  * Additional options for scaleFit().
//  *
//  * invertY: if true, flips direction of growth of Y axis.
//  * zoom: modifier applied to scale after exact fit calculation.
//  */
// // TODO: Consider unifying StretchOptions and ScaleOptions into one object and adding inverX for completeness.
// // export type FitOptions = {
// //     invertY?: boolean,
// //     zoom?: number;
// // };
// export class FitOptions {
//     constructor(public invertY?: boolean, public zoom?: number) {}
// }


// /** Find the scaling parameters that allow inner to fit inside outer with uniform scaling. */
// export const ScaleFit = (inner: Rect, outer: Rect, options: FitOptions = {}): AffineTransform => {
//     const invertY = (options.invertY === undefined) ? false : options.invertY;
//     const zoom    = (options.zoom    === undefined) ? 1.0   : options.zoom;

//     const flipY = invertY ? -1 : +1;

//     const diagIn  = inner.Diagonal();
//     const diagOut = outer.Diagonal();

//     const ci = inner.Center();
//     const co = outer.Center();

//     const sx = diagOut.x / diagIn.x;
//     const sy = diagOut.y / diagIn.y;
//     const s  = Math.min(sx, sy) * zoom;

//     const ox = co.x - s * ci.x;
//     const oy = co.y - flipY * s * ci.y;

//     return { offset: new Vec2(ox, oy), scale: new Vec2(s, invertY ? -s : +s) };
// }
