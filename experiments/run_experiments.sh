REGULAR=false
EMBEDDING=false
RELATIONS=false
DRY_RUN=""
NAME="ruse_experiments"

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        --regular)
            REGULAR=true
            shift
            ;;
        --relations)
            RELATIONS=true
            shift
            ;;
        --embedding)
            EMBEDDING=true
            shift
            ;;
        --name)
            NAME="$2"
            shift
            shift
            ;;
    esac
done

if [ "$RELATIONS" = true ]; then
    echo "Running relations benchmarks"
    bash ./run_ruse_relations_benchmarks.sh ${NAME}_relations
fi

if [ "$REGULAR" = true ]; then
    echo "Running regular benchmarks"
    bash ./run_ruse_regular_benchmarks.sh ${NAME}_regular
fi

if [ "$EMBEDDING" = true ]; then
    echo "Running embedding benchmarks"
    bash ./run_ruse_embedding_wide_overhead.sh ${NAME}_wide_embedding
    bash ./run_ruse_embedding_graph_overhead.sh ${NAME}_graph_embedding
fi
