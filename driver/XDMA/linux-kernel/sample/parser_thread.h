#ifndef __PARSER_THREAD_H__
#define __PARSER_THREAD_H__

#include "buffer_handler.h"

typedef struct circular_parsed_queue{
    struct xdma_buffer_descriptor elements[NUMBER_OF_QUEUE];
    int front;
    int rear;
    int count;
    pthread_mutex_t mutex;
} CircularParsedQueue_t;

int pbuffer_multi_dequeue(CircularParsedQueue_t *queue, struct xdma_multi_read_write_ioctl *bd);
int getParsedQueueCount(CircularParsedQueue_t * q);


#endif /* __PARSER_THREAD_H__ */
