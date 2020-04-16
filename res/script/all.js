/* Expand a post's image when it is clicked. */
function onClickPostImage(ev) {
    if (ev.target.dataset.expanded) {
        delete ev.target.dataset.expanded
        ev.target.src = ev.target.dataset.thumbUri
    } else if (ev.target.dataset.expanding) {
        delete ev.target.dataset.expanding
        ev.target.src = ev.target.dataset.thumbUri
    } else {
        ev.target.dataset.expanding = "expanding"
        ev.target.src = ev.target.dataset.uri
    }
}

/* Update the image's attributes once the image is done loading. */
function onLoadPostImage(ev) {
    if (ev.target.dataset.expanding) {
        delete ev.target.dataset.expanding
        ev.target.dataset.expanded = "expanded"
    }
}

/* Display a preview of a post when the cursor hovers over a post reference. */
function onMouseEnterPostRef(ev) {
    var targetRect = ev.target.getBoundingClientRect()

    var id = ev.target.textContent.replace(">>", "")

    var post = document.getElementById(id)

    if (post === null) {
        var url = window.location.href.replace(/#.*/gi, "")
        url += "/preview/" + id

        fetch(url)
            .then((response) => {
                return response.text()
            })
            .then((content) => {
                var template = document.createElement('template')
                template.innerHTML = content

                addPostPreview(template.content.firstChild, targetRect)
            })
    } else {
        addPostPreview(post.cloneNode(true), targetRect)
    }
}

/* Insert a post preview into the DOM. */
function addPostPreview(postPreview, targetRect) {
    postPreview.classList.add("post-fixed")

    document.body.appendChild(postPreview)

    var previewRect = postPreview.getBoundingClientRect()

    var maxPreviewTop = window.innerHeight - previewRect.height - 40

    var previewTop = targetRect.bottom - (previewRect.height / 2)
    previewTop = Math.max(previewTop, 0)
    previewTop = Math.min(previewTop, maxPreviewTop)

    var previewMaxWidth = window.innerWidth - targetRect.right - 40

    postPreview.style.top = previewTop + "px"
    postPreview.style.left = (targetRect.right + 10) + "px"
    postPreview.style.maxWidth = previewMaxWidth + "px"
}

/* Remove a post preview when the cursor stops hovering over the post
 * reference. */
function onMouseLeavePostRef(ev) {
    document.querySelectorAll('.post-fixed').forEach((elem) => {
        elem.remove()
    })
}

document.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('.post-image img').forEach((elem) => {
        elem.addEventListener('click', onClickPostImage)
        elem.addEventListener('load', onLoadPostImage)
    })

    document.querySelectorAll('.post-ref').forEach((elem) => {
        elem.addEventListener('mouseenter', onMouseEnterPostRef)
    })

    document.querySelectorAll('.post-ref').forEach((elem) => {
        elem.addEventListener('mouseleave', onMouseLeavePostRef)
    })
})
