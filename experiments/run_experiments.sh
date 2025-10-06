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

if [ "$REGULAR" = true ]; then
    echo "Running regular benchmarks"
    bash ./run_regular_benchmarks_ruse.sh ${NAME}_regular_benchmarks_results
fi

if [ "$RELATIONS" = true ]; then
    echo "Running relations benchmarks"
    bash ./run_relations_benchmarks_ruse.sh ${NAME}_relations_benchmarks_results
fi

if [ "$EMBEDDING" = true ]; then
    echo "Running embedding benchmarks"
    bash ./run_embedding_benchmarks_ruse.sh ${NAME}_embedding_benchmarks_results
fi
