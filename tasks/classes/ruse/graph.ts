export class GraphNode {
    public value: number;
    public neighbors: GraphNode[];

    public constructor(value: number, ...neighbors: GraphNode[]) {
        this.value = value;
        this.neighbors = neighbors;
    }

    public inc_value(delta: number): void {
        this.value += delta;
    }
    
    public add_neighbor(neighbor: GraphNode): void {
        this.neighbors.push(neighbor);
    }
}
